//! Tests for the bonecoin wallet

use super::*;
use std::time::Instant; // TODO Import allowed?

fn empty_wallet() -> Wallet {
    Wallet::new(vec![].into_iter())
}

fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}

fn wallet_with_alice_and_bob_and_charlie() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob, Address::Charlie].into_iter())
}

#[test]
fn correct_genesis_values() {
    let wallet = wallet_with_alice();

    assert_eq!(wallet.best_height(), 0);
    assert_eq!(wallet.best_hash(), Block::genesis().id());
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice).unwrap().len(), 0);
}

#[test]
fn foreign_address_error() {
    let wallet = wallet_with_alice();

    assert_eq!(
        wallet.total_assets_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
}

#[test]
fn no_address_error() {
    let wallet = empty_wallet();

    assert_eq!(
        wallet.create_manual_transaction(Vec::new(), Vec::new()),
        Err(WalletError::NoOwnedAddresses)
    );
    assert_eq!(
        wallet.create_automatic_transaction(Address::Eve, 0, 0),
        Err(WalletError::NoOwnedAddresses)
    );
}

#[test]
fn empty_manual_transaction() {
    let wallet = wallet_with_alice();

    let result = wallet.create_manual_transaction(vec![], vec![]);
    assert!(result.is_ok());
    let transaction = result.unwrap().clone();
    assert_eq!(transaction.inputs.len(), 0);
    assert_eq!(transaction.outputs.len(), 0);
}

#[test]
fn empty_automatic_transaction() {
    let wallet = wallet_with_alice();

    let result = wallet.create_automatic_transaction(Address::Eve, 0, 0);
    assert!(result.is_ok());
    let transaction = result.unwrap().clone();
    assert_eq!(transaction.inputs.len(), 0);
    assert_eq!(transaction.outputs.len(), 0);
}

#[test]
fn sync_two_blocks() {
    // Build a mock node that has a simple two block chain
    let mut node = MockNode::new();
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

#[test]
fn short_reorg() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 1
    let _old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    wallet.sync(&node);

    // Reorg to longer chain of length 2
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

#[test]
fn deep_reorg() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    // Reorg to longer chain of length 5
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);
}

#[test]
fn reorg_to_shorter_chain() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    // Reorg to shorter chain of length 2
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);
}

#[test]
fn tracks_single_utxo() {
    // We have a single transaction that consumes some made up input
    // and creates a single output to alice.
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(vec![(coin_id, COIN_VALUE)])
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
}

#[test]
fn consumes_own_utxo() {
    // All coins will be valued the same in this test
    const COIN_VALUE: u64 = 100;

    // We start by minting a coin to alice
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx_mint = Transaction {
        inputs: vec![],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx_mint.coin_id(0);

    // Then we burn that coin
    let input = Input {
        coin_id,
        // The signature is invalid to save syntax.
        // The wallet doesn't check validity anyway.
        // This transaction is in a block, so the wallet syncs it.
        signature: Signature::Invalid,
    };
    let tx_burn = Transaction {
        inputs: vec![input],
        outputs: vec![],
    };

    // Apply this all to a blockchain and sync the wallet.
    // We apply in two separate blocks although that shouldn't be necessary.
    let mut node = MockNode::new();
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![tx_mint]);
    let _b2_id = node.add_block_as_best(b1_id, vec![tx_burn]);
    let mut wallet = wallet_with_alice();
    wallet.sync(&node);

    // Make sure the UTXO is consumed
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(vec![]));
    // Pedagogy: It is reasonable that the wallet could provide details about
    // the coin even after it was spent. But requiring that gives away the trick of
    // tracking spent coins so you can revert them later.
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));
}

// Track UTXOs from two transactions in a block
// Track UTXOs to multiple users
#[test]
fn tracks_utxo_from_two_tx_in_one_block_to_multiple_users() {
    // We have a single transaction that consumes some made up input
    // and creates a single output to alice.
    const COIN_VALUE: u64 = 100;
    let coin1 = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let coin2 = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };
    let tx1 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin1.clone()],
    };

    let tx2 = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin2.clone()],
    };
    let coin_id1: CoinId = tx1.coin_id(0);
    let coin_id2: CoinId = tx2.coin_id(0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx1, tx2]);

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE * 2);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(vec![(coin_id1, COIN_VALUE)])
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Ok(vec![(coin_id2, COIN_VALUE)])
    );
    assert_eq!(wallet.coin_details(&coin_id1), Ok(coin1));
    assert_eq!(wallet.coin_details(&coin_id2), Ok(coin2));
}

// Create manual transaction
// ... with missing input
// ... with too much output
#[test]
fn manual_tx_happy_flow() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);
    let input_coin_ids = vec![coin_id];
    let coin_bob = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };
    let output_coins = vec![coin_bob];
    let tx_result = wallet.create_manual_transaction(input_coin_ids, output_coins.clone());
    assert!(&tx_result.is_ok());
    let result = tx_result.unwrap().clone();
    assert_eq!(result.outputs, output_coins);
    assert_eq!(result.inputs[0].signature, Signature::Valid(Address::Alice));
}

#[test]
fn manual_tx_missing_input() {
    const COIN_VALUE: u64 = 100;
    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), Vec::new());

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);

    let dummy_input = Input::dummy();
    let input_coin_ids = vec![dummy_input.coin_id];
    let coin_bob = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };
    let output_coins = vec![coin_bob];
    let tx_result = wallet.create_manual_transaction(input_coin_ids, output_coins.clone());
    assert!(tx_result.is_err());
    assert_eq!(tx_result.unwrap_err(), WalletError::UnknownCoin);
}

#[test]
fn manual_tx_too_much_output() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);
    let input_coin_ids = vec![coin_id];
    let coin_bob = Coin {
        value: COIN_VALUE,
        owner: Address::Bob,
    };
    let coin_alice = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let output_coins = vec![coin_bob, coin_alice];
    let tx_result = wallet.create_manual_transaction(input_coin_ids, output_coins.clone());
    assert!(tx_result.is_err());
    assert_eq!(tx_result.unwrap_err(), WalletError::OutputsExceedInputs);
}

// Create automatic transactions
// ... with too much output
#[test]
fn automatic_tx_happy_flow_zero_tip() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(0);

    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);
    let coin_charlie = Coin {
        value: COIN_VALUE,
        owner: Address::Charlie,
    };
    let output_coins = vec![coin_charlie];
    let tx_result = wallet.create_automatic_transaction(Address::Charlie, COIN_VALUE, 0);
    assert!(tx_result.is_ok());
    let results = tx_result.unwrap();
    assert_eq!(results.outputs, output_coins);
    assert_eq!(results.inputs[0].coin_id, coin_id);
    assert_eq!(
        results.inputs[0].signature,
        Signature::Valid(Address::Alice)
    );
}

#[test]
fn automatic_tx_too_much_output() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    // Create a minimal chain to contain this transaction and sync it
    let mut node = MockNode::new();
    node.add_block_as_best(Block::genesis().id(), vec![tx]);

    let mut wallet = wallet_with_alice_and_bob();
    wallet.sync(&node);

    let tx_result = wallet.create_automatic_transaction(Address::Charlie, 2 * COIN_VALUE, 0);
    assert!(tx_result.is_err());
    // assert_eq!(tx_result.unwrap().outputs, output_coins);
}

// Reorgs with utxos in the chain history
#[test]
fn reorg_with_utxos_input() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // We have a single transaction that consumes some made up input
    // and creates a single output to alice.
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = tx.coin_id(0);

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![tx]);
    wallet.sync(&node);

    let coin = wallet.coin_details(&coin_id).unwrap();

    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), old_b3_id);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(vec![(coin_id, coin.value)])
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));

    // Reorg to shorter chain of length 2
    let b1_id = node.add_block(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 2);
    assert_eq!(wallet.best_hash(), b2_id);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(vec![]));
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));
}

// Reorgs with utxos in the chain history
#[test]
fn reorg_with_utxos_output() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let mint_tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = mint_tx.coin_id(0);

    let spend_tx = Transaction {
        inputs: vec![Input {
            coin_id: coin_id.clone(),
            signature: Signature::Invalid,
        }],
        outputs: vec![],
    };

    // Sync a chain to height 5
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);

    let old_b4_id = node.add_block_as_best(old_b3_id, vec![spend_tx]);
    let _old_b5_id = node.add_block_as_best(old_b4_id, vec![]);
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(vec![]));
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));

    // Reorg to longer chain of same length
    let b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(vec![(coin_id, COIN_VALUE)])
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
}

// Reorgs with utxos in the chain history
#[test]
fn reorg_with_utxos_complete() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice_and_bob();

    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let mint_tx = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin.clone()],
    };
    let coin_id = mint_tx.coin_id(0);

    let spend_tx = Transaction {
        inputs: vec![Input {
            coin_id: coin_id.clone(),
            signature: Signature::Invalid,
        }],
        outputs: vec![Coin {
            value: COIN_VALUE,
            owner: Address::Bob,
        }],
    };

    // Sync a chain to height 5
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);

    let old_b4_id = node.add_block_as_best(old_b3_id, vec![spend_tx]);
    let _old_b5_id = node.add_block_as_best(old_b4_id, vec![]);
    wallet.sync(&node);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(wallet.all_coins_of(Address::Alice), Ok(vec![]));
    assert_eq!(wallet.coin_details(&coin_id), Err(WalletError::UnknownCoin));

    // Reorg to longer chain of same length
    let b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);

    // Check that the accounting is right
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(0));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(vec![(coin_id, COIN_VALUE)])
    );
    assert_eq!(wallet.all_coins_of(Address::Bob), Ok(vec![]));
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
}

// Reorg performance tests to make sure they aren't just syncing from genesis each time.
#[test]
fn reorg_performance_test() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 5
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);

    let old_b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let _old_b5_id = node.add_block_as_best(old_b4_id, vec![]);
    wallet.sync(&node);

    // Reorg to longer chain of same length
    let b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    let now = Instant::now();
    wallet.sync(&node);
    let elapsed_reorg = now.elapsed().as_nanos();

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);

    // Now create the same chain from scratch and sync.
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    let b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let _b5_id = node.add_block_as_best(b4_id, vec![]);
    let now = Instant::now();
    wallet.sync(&node);
    let elapsed_full_sync = now.elapsed().as_nanos();

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);
    assert!(elapsed_full_sync > elapsed_reorg);
}

/// Creates a simple transaction that is unlikely to conflict with any other
/// transactions in your tests. This is useful to when you are intentionally creating
/// a fork. By including a marker transaction on one side of the fork, but not the other,
/// you make sure that the two chains are truly different.
fn marker_tx() -> Transaction {
    Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 123,
            owner: Address::Custom(123),
        }],
    }
}

#[test]
fn deep_reorg_new_version() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice_and_bob_and_charlie();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    // Reorg to longer chain of length 5

    // In the original test, b1 was identical to old_b1 (and same for b2 and b3)
    // This means that was wasn't really a fork or re-org at all. I just extended
    // the same old chain. By including a marker transaction on one side of the fork,
    // we make sure the blocks are truly unique. It is not necessary to include the marker
    // in all the descendants. Once we have modified a single block all descendants will
    // also be modified because of the parent pointers. Do this for all of your re-org tests.
    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);
}

#[test]
fn deep_reorg_new_version_really_long() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let old_b2_id = node.add_block_as_best(old_b1_id, vec![]);
    let old_b3_id = node.add_block_as_best(old_b2_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 3);
    assert_eq!(wallet.best_hash(), old_b3_id);

    // Reorg to longer chain of length 5
    let b1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);

    // Reorg to longer chain of length 10
    let new_b4_id = node.add_block_as_best(b3_id, vec![marker_tx()]);
    let new_b5_id = node.add_block_as_best(new_b4_id, vec![]);
    let new_b6_id = node.add_block_as_best(new_b5_id, vec![]);
    let new_b7_id = node.add_block_as_best(new_b6_id, vec![]);
    let new_b8_id = node.add_block_as_best(new_b7_id, vec![]);
    let new_b9_id = node.add_block_as_best(new_b8_id, vec![]);
    let new_b10_id = node.add_block_as_best(new_b9_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 10);
    assert_eq!(wallet.best_hash(), new_b10_id);
}

#[test]
fn crazy_reorg() {
    // Create node and wallet
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    const COIN_VALUE: u64 = 100;
    let coin_alice = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let coin_bob = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };
    let coin_charlie = Coin {
        value: COIN_VALUE,
        owner: Address::Alice,
    };

    let tx: Transaction = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_alice.clone(), coin_bob.clone()],
    };
    let tx2: Transaction = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_alice.clone(), coin_alice.clone()],
    };
    let tx3: Transaction = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![coin_alice.clone(), coin_charlie.clone()],
    };
    let tx4: Transaction = Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![
            coin_charlie.clone(),
            coin_bob.clone(),
            coin_charlie.clone(),
            coin_bob.clone(),
        ],
    };

    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    let b6_id = node.add_block_as_best(b5_id, vec![]);
    let b7_id = node.add_block_as_best(b6_id, vec![tx.clone()]);
    let b8_id = node.add_block_as_best(b7_id, vec![]);
    let b9_id = node.add_block_as_best(b8_id, vec![tx.clone()]);
    let b10_id = node.add_block_as_best(b9_id, vec![]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 10);
    assert_eq!(wallet.best_hash(), b10_id);

    let b11_id = node.add_block_as_best(b4_id, vec![marker_tx()]);
    let b12_id = node.add_block_as_best(b11_id, vec![]);
    let b13_id = node.add_block_as_best(b12_id, vec![tx2.clone()]);
    let b14_id = node.add_block_as_best(b13_id, vec![]);
    let b15_id = node.add_block_as_best(b14_id, vec![]);
    let b16_id = node.add_block_as_best(b15_id, vec![tx3.clone()]);
    let b17_id = node.add_block_as_best(b16_id, vec![]);
    let b18_id = node.add_block_as_best(b17_id, vec![]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 12);
    assert_eq!(wallet.best_hash(), b18_id);

    let b19_id = node.add_block_as_best(b14_id, vec![marker_tx(), marker_tx()]);
    let b20_id = node.add_block_as_best(b19_id, vec![]);
    let b21_id = node.add_block_as_best(b20_id, vec![tx.clone()]);
    let b22_id = node.add_block_as_best(b21_id, vec![]);
    let b23_id = node.add_block_as_best(b22_id, vec![]);
    let b24_id = node.add_block_as_best(b23_id, vec![]);
    let b25_id = node.add_block_as_best(b24_id, vec![tx.clone()]);
    let b26_id = node.add_block_as_best(b25_id, vec![]);
    let b27_id = node.add_block_as_best(b26_id, vec![]);
    let b28_id = node.add_block_as_best(b27_id, vec![tx.clone()]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 18);
    assert_eq!(wallet.best_hash(), b28_id);

    let b29_id =
        node.add_block_as_best(b8_id, vec![tx.clone(), tx.clone(), tx.clone(), tx.clone()]);
    let b30_id = node.add_block_as_best(b29_id, vec![]);
    let b31_id = node.add_block_as_best(b30_id, vec![]);
    let b32_id = node.add_block_as_best(b31_id, vec![]);
    let b33_id = node.add_block_as_best(b32_id, vec![]);
    let b34_id = node.add_block_as_best(b33_id, vec![]);
    let b35_id = node.add_block_as_best(b34_id, vec![]);
    let b36_id = node.add_block_as_best(b35_id, vec![tx.clone(), tx4.clone()]);
    let b37_id = node.add_block_as_best(b36_id, vec![]);
    let b38_id = node.add_block_as_best(b37_id, vec![]);
    let b39_id = node.add_block_as_best(b38_id, vec![]);
    let b40_id = node.add_block_as_best(b39_id, vec![tx2.clone()]);
    let b41_id = node.add_block_as_best(b40_id, vec![]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 21);
    assert_eq!(wallet.best_hash(), b41_id);

    let b42_id = node.add_block_as_best(b39_id, vec![tx3.clone()]);
    let b43_id = node.add_block_as_best(b42_id, vec![]);
    let b44_id = node.add_block_as_best(b43_id, vec![tx4.clone()]);
    let b45_id = node.add_block_as_best(b44_id, vec![]);
    let b46_id = node.add_block_as_best(b45_id, vec![]);
    let b47_id = node.add_block_as_best(b46_id, vec![]);
    let b48_id = node.add_block_as_best(
        b47_id,
        vec![tx.clone(), tx2.clone(), tx3.clone(), tx4.clone()],
    );
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 26);
    assert_eq!(wallet.best_hash(), b48_id);

    let b49_id = node.add_block_as_best(b45_id, vec![]);
    let b50_id = node.add_block_as_best(b49_id, vec![]);
    let b51_id = node.add_block_as_best(b50_id, vec![]);
    let b52_id = node.add_block_as_best(b51_id, vec![tx.clone()]);
    let b53_id = node.add_block_as_best(b52_id, vec![]);
    let b54_id = node.add_block_as_best(b53_id, vec![]);
    let b55_id = node.add_block_as_best(b54_id, vec![]);
    let b56_id = node.add_block_as_best(b55_id, vec![]);
    let b57_id = node.add_block_as_best(b56_id, vec![]);
    let b58_id = node.add_block_as_best(b57_id, vec![tx.clone()]);
    wallet.sync(&node);
    assert_eq!(wallet.best_height(), 33);
    assert_eq!(wallet.best_hash(), b58_id);

    let mut no_reorg_node = MockNode::new();
    let mut no_reorg_wallet = wallet_with_alice();
    // Similar to previous chain but only path from genesis to b58
    let b1_id = no_reorg_node.add_block_as_best(Block::genesis().id(), vec![]);
    let b2_id = no_reorg_node.add_block_as_best(b1_id, vec![]);
    let b3_id = no_reorg_node.add_block_as_best(b2_id, vec![]);
    let b4_id = no_reorg_node.add_block_as_best(b3_id, vec![]);
    let b5_id = no_reorg_node.add_block_as_best(b4_id, vec![]);
    let b6_id = no_reorg_node.add_block_as_best(b5_id, vec![]);
    let b7_id = no_reorg_node.add_block_as_best(b6_id, vec![tx.clone()]);
    let b8_id = no_reorg_node.add_block_as_best(b7_id, vec![]);
    let b29_id = no_reorg_node
        .add_block_as_best(b8_id, vec![tx.clone(), tx.clone(), tx.clone(), tx.clone()]);
    let b30_id = no_reorg_node.add_block_as_best(b29_id, vec![]);
    let b31_id = no_reorg_node.add_block_as_best(b30_id, vec![]);
    let b32_id = no_reorg_node.add_block_as_best(b31_id, vec![]);
    let b33_id = no_reorg_node.add_block_as_best(b32_id, vec![]);
    let b34_id = no_reorg_node.add_block_as_best(b33_id, vec![]);
    let b35_id = no_reorg_node.add_block_as_best(b34_id, vec![]);
    let b36_id = no_reorg_node.add_block_as_best(b35_id, vec![tx.clone(), tx4.clone()]);
    let b37_id = no_reorg_node.add_block_as_best(b36_id, vec![]);
    let b38_id = no_reorg_node.add_block_as_best(b37_id, vec![]);
    let b39_id = no_reorg_node.add_block_as_best(b38_id, vec![]);
    let b42_id = no_reorg_node.add_block_as_best(b39_id, vec![tx3.clone()]);
    let b43_id = no_reorg_node.add_block_as_best(b42_id, vec![]);
    let b44_id = no_reorg_node.add_block_as_best(b43_id, vec![tx4.clone()]);
    let b45_id = no_reorg_node.add_block_as_best(b44_id, vec![]);
    let b49_id = no_reorg_node.add_block_as_best(b45_id, vec![]);
    let b50_id = no_reorg_node.add_block_as_best(b49_id, vec![]);
    let b51_id = no_reorg_node.add_block_as_best(b50_id, vec![]);
    let b52_id = no_reorg_node.add_block_as_best(b51_id, vec![tx.clone()]);
    let b53_id = no_reorg_node.add_block_as_best(b52_id, vec![]);
    let b54_id = no_reorg_node.add_block_as_best(b53_id, vec![]);
    let b55_id = no_reorg_node.add_block_as_best(b54_id, vec![]);
    let b56_id = no_reorg_node.add_block_as_best(b55_id, vec![]);
    let b57_id = no_reorg_node.add_block_as_best(b56_id, vec![]);
    let b58_id = no_reorg_node.add_block_as_best(b57_id, vec![tx.clone()]);
    no_reorg_wallet.sync(&no_reorg_node);
    assert_eq!(no_reorg_wallet.best_height(), 33);
    assert_eq!(no_reorg_wallet.best_hash(), b58_id);

    assert_eq!(wallet.best_hash(), no_reorg_wallet.best_hash());
    assert_eq!(
        wallet.total_assets_of(Address::Alice),
        no_reorg_wallet.total_assets_of(Address::Alice)
    );
    assert_eq!(
        wallet.total_assets_of(Address::Bob),
        no_reorg_wallet.total_assets_of(Address::Bob)
    );
    assert_eq!(
        wallet.total_assets_of(Address::Charlie),
        no_reorg_wallet.total_assets_of(Address::Charlie)
    );
    assert_eq!(wallet.net_worth(), no_reorg_wallet.net_worth());
}
