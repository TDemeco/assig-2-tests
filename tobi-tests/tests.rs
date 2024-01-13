//! Tests for the bonecoin wallet

use super::*;

fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}

fn wallet_with_addresses(addresses: Vec<Address>) -> Wallet {
    Wallet::new(addresses.into_iter())
}

fn multiple_mint(addresses: Vec<Address>, values: Vec<u64>) -> Transaction {
    if addresses.len() != values.len() {
        panic!("Address quantity should be equal to values quantity"); // Just in case, we should never get here
    }

    let addresses_with_values: Vec<(Address, u64)> =
        addresses.into_iter().zip(values.into_iter()).collect();

    let mut coin_vector: Vec<Coin> = Vec::new();
    for (address, value) in addresses_with_values {
        coin_vector.push(Coin {
            owner: address,
            value,
        });
    }

    Transaction {
        inputs: vec![],
        outputs: coin_vector,
    }
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
    assert!(tx_result.is_ok());
    assert_eq!(tx_result.unwrap().outputs, output_coins);
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
    let _old_b3_id = node.add_block_as_best(old_b2_id, vec![tx]);
    wallet.sync(&node);

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
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(100));
    assert_eq!(wallet.net_worth(), 100);
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

#[test]
fn automatic_tx_multi_account_zero_tip() {
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

    let charlie_amount = COIN_VALUE * 2;
    let coin_charlie = Coin {
        value: charlie_amount,
        owner: Address::Charlie,
    };
    let output_coins = vec![coin_charlie];
    let tx_result = wallet.create_automatic_transaction(Address::Charlie, charlie_amount, 0);
    assert!(tx_result.is_ok());
    let results = tx_result.unwrap();
    assert_eq!(results.outputs, output_coins);
    assert!(results.inputs.contains(&Input {
        coin_id: coin_id1,
        signature: Signature::Valid(Address::Alice)
    }));
    assert!(results.inputs.contains(&Input {
        coin_id: coin_id2,
        signature: Signature::Valid(Address::Bob)
    }));
}

#[test]
fn automatic_tx_remaining_amount_back_and_tip() {
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

    let to_return = 20;
    let tip = 10;
    let charlie_amount = COIN_VALUE - to_return - tip;
    let coin_charlie = Coin {
        value: charlie_amount,
        owner: Address::Charlie,
    };

    let tx_result = wallet.create_automatic_transaction(Address::Charlie, charlie_amount, tip);
    assert!(tx_result.is_ok());
    let results = tx_result.unwrap();
    assert_eq!(results.outputs.len(), 2);
    assert_eq!(results.outputs[0], coin_charlie.clone());
    assert_eq!(results.outputs[1].value, to_return); // The retunred amount can be to any address in the wallet, we care about the amount in this case
    assert_eq!(results.inputs[0].coin_id, coin_id);
}

#[test]
fn manual_tx_happy_flow_send_to_non_wallet_address() {
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
    let coin_charlie = Coin {
        value: COIN_VALUE,
        owner: Address::Charlie,
    };
    let output_coins = vec![coin_charlie];
    let tx_result = wallet.create_manual_transaction(input_coin_ids, output_coins.clone());
    assert!(tx_result.is_ok());
    assert_eq!(tx_result.unwrap().outputs, output_coins);
}

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

    let mut n_queries = node.how_many_queries();
    println!("n_queries: {}", n_queries);
    assert!(n_queries <= 7);
    println!("finished first sync");
    println!("==============================================");
    // Reorg to longer chain of same length
    let b4_id = node.add_block_as_best(old_b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);
    n_queries = node.how_many_queries();
    println!("n_queries: {}", n_queries);
    assert!(n_queries <= 9);

    println!("finished second sync");
    println!("==============================================");

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
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), b5_id);
    assert!(node.how_many_queries() == 7);
}

/// OWN TESTS:
///
/// This next test advances the chain up, including transactions in each block such as minting,
/// transfering with tip, transfering without tip and burning, keeping track of those for each address
/// in the wallet. Then, a reorg happens, and we test the ability of the wallet to roll back the now-invalid
/// transactions and arrive at the same state as the node
#[test]
fn deep_reorg_with_mint_spend_burn() {
    // Create node and wallet
    let mut node = MockNode::new();

    // Create wallet with Alice
    let mut alice_wallet = wallet_with_alice();
    // Create a wallet with Bob, Charlie and Custom(100)
    let mut multi_wallet =
        wallet_with_addresses(vec![Address::Bob, Address::Charlie, Address::Custom(100)]);

    // Sync a chain to height 5, minting and transacting in the process
    let mint_tx = multiple_mint(
        vec![
            Address::Alice,
            Address::Bob,
            Address::Charlie,
            Address::Custom(100),
            Address::Eve,
        ],
        vec![100, 2000, 300, 50, 1234],
    );
    let alice_coins = mint_tx.coin_id(0);
    let bob_coins = mint_tx.coin_id(1);
    let charlie_coins = mint_tx.coin_id(2);
    let custom_address_coins = mint_tx.coin_id(3);

    let old_b1_id = node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    let old_b2_id = node.add_block_as_best(
        old_b1_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: alice_coins,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![Coin {
                owner: Address::Eve,
                value: 50,
            }],
        }],
    );
    let old_b3_id = node.add_block_as_best(
        old_b2_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: charlie_coins,
                signature: Signature::Valid(Address::Charlie),
            }],
            outputs: vec![Coin {
                owner: Address::Alice,
                value: 150,
            }],
        }],
    );
    let old_b4_id = node.add_block_as_best(
        old_b3_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: bob_coins,
                signature: Signature::Valid(Address::Bob),
            }],
            outputs: vec![
                Coin {
                    owner: Address::Alice,
                    value: 300,
                },
                Coin {
                    owner: Address::Bob,
                    value: 1700,
                },
            ],
        }],
    );
    let _old_b5_id = node.add_block_as_best(
        old_b4_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: custom_address_coins,
                signature: Signature::Valid(Address::Custom(100)),
            }],
            outputs: vec![],
        }],
    );
    alice_wallet.sync(&node);
    multi_wallet.sync(&node);
    // Up to here, we have a chain with 6 blocks and balances:
    // Alice = 450 (one 150 UTXO and one 300 UTXO)
    let alice_coins = alice_wallet.all_coins_of(Address::Alice).unwrap();
    assert_eq!(alice_coins[0].1, 150);
    assert_eq!(alice_coins[1].1, 300);
    assert_eq!(alice_wallet.total_assets_of(Address::Alice).unwrap(), 450);
    // Bob = 1700 (one UTXO)
    let bob_coins = multi_wallet.all_coins_of(Address::Bob).unwrap();
    assert_eq!(bob_coins[0].1, 1700);
    assert_eq!(multi_wallet.total_assets_of(Address::Bob).unwrap(), 1700);
    // Charlie = 0
    let charlie_coins = multi_wallet.all_coins_of(Address::Charlie).unwrap();
    assert!(charlie_coins.is_empty());
    assert_eq!(multi_wallet.total_assets_of(Address::Charlie).unwrap(), 0);
    // Custom(100) = 0 (burned his coins)
    let custom_address_coins = multi_wallet.all_coins_of(Address::Custom(100)).unwrap();
    assert!(custom_address_coins.is_empty());
    assert_eq!(
        multi_wallet.total_assets_of(Address::Custom(100)).unwrap(),
        0
    );
    // Eve = 1284 (although we won't care about Eve because we don't own her)

    // Reorg to longer chain of length 7 (whose common parent is block number 1, not genesis)
    let b2_id = node.add_block_as_best(old_b1_id, vec![marker_tx()]);
    let b3_id = node.add_block_as_best(b2_id, vec![]);
    let b4_id = node.add_block_as_best(b3_id, vec![]);
    let b5_id = node.add_block_as_best(b4_id, vec![]);
    let b6_id = node.add_block_as_best(b5_id, vec![]);
    alice_wallet.sync(&node);
    multi_wallet.sync(&node);
    // Now, we should have a chain where balances are the ones minted in block number 1
    // Alice = 100 (only one UTXO)
    let alice_coins: Vec<(CoinId, u64)> = alice_wallet.all_coins_of(Address::Alice).unwrap();
    assert_eq!(alice_coins[0].1, 100);
    assert_eq!(alice_wallet.total_assets_of(Address::Alice).unwrap(), 100);
    // Bob = 2000
    let bob_coins = multi_wallet.all_coins_of(Address::Bob).unwrap();
    assert_eq!(bob_coins[0].1, 2000);
    assert_eq!(multi_wallet.total_assets_of(Address::Bob).unwrap(), 2000);
    // Charlie = 300
    let charlie_coin = multi_wallet.all_coins_of(Address::Charlie).unwrap();
    assert_eq!(charlie_coin[0].1, 300);
    assert_eq!(multi_wallet.total_assets_of(Address::Charlie).unwrap(), 300);
    // Custom(100) = 50
    let custom_address_coin = multi_wallet.all_coins_of(Address::Custom(100)).unwrap();
    assert_eq!(custom_address_coin[0].1, 50);
    assert_eq!(
        multi_wallet.total_assets_of(Address::Custom(100)).unwrap(),
        50
    );
    // Eve = 1234

    assert_eq!(alice_wallet.best_height(), 6);
    assert_eq!(alice_wallet.best_hash(), b6_id);
    assert_eq!(multi_wallet.best_height(), 6);
    assert_eq!(multi_wallet.best_hash(), b6_id);
}

/// This next test advances the chain up, including transactions in each block such as minting,
/// transfering with tip, transfering without tip and burning, including for an address that the wallet
/// didn't previously owned (Eve). Then, we add that address and check if its data was correctly updated
#[test]
fn add_owned_address_test() {
    // Create node and wallet
    let mut node = MockNode::new();

    // Create wallet with Alice
    let mut alice_wallet = wallet_with_alice();
    // Create a wallet with Bob, Charlie and Custom(100)
    let mut multi_wallet =
        wallet_with_addresses(vec![Address::Bob, Address::Charlie, Address::Custom(100)]);

    // Sync a chain to height 5, minting and transacting in the process
    let mint_tx = multiple_mint(
        vec![
            Address::Alice,
            Address::Bob,
            Address::Charlie,
            Address::Custom(100),
            Address::Eve,
        ],
        vec![100, 2000, 300, 50, 1234],
    );
    let alice_coins = mint_tx.coin_id(0);
    let bob_coins = mint_tx.coin_id(1);
    let charlie_coins = mint_tx.coin_id(2);
    let custom_address_coins = mint_tx.coin_id(3);

    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    let b2_id = node.add_block_as_best(
        b1_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: alice_coins,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![Coin {
                owner: Address::Eve,
                value: 50,
            }],
        }],
    );
    let b3_id = node.add_block_as_best(
        b2_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: charlie_coins,
                signature: Signature::Valid(Address::Charlie),
            }],
            outputs: vec![Coin {
                owner: Address::Alice,
                value: 150,
            }],
        }],
    );
    let b4_id = node.add_block_as_best(
        b3_id,
        vec![
            Transaction {
                inputs: vec![Input {
                    coin_id: bob_coins,
                    signature: Signature::Valid(Address::Bob),
                }],
                outputs: vec![
                    Coin {
                        owner: Address::Alice,
                        value: 300,
                    },
                    Coin {
                        owner: Address::Bob,
                        value: 1700,
                    },
                ],
            },
            Transaction {
                inputs: vec![],
                outputs: vec![Coin {
                    owner: Address::Eve,
                    value: 16,
                }],
            },
        ],
    );
    let _b5_id = node.add_block_as_best(
        b4_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: custom_address_coins,
                signature: Signature::Valid(Address::Custom(100)),
            }],
            outputs: vec![],
        }],
    );
    alice_wallet.sync(&node);
    multi_wallet.sync(&node);
    // Up to here, we have a chain with 6 blocks and balances:
    // Alice = 450 (one 150 UTXO and one 300 UTXO)
    let alice_coins = alice_wallet.all_coins_of(Address::Alice).unwrap();
    assert_eq!(alice_coins[0].1, 150);
    assert_eq!(alice_coins[1].1, 300);
    assert_eq!(alice_wallet.total_assets_of(Address::Alice).unwrap(), 450);
    // Bob = 1700 (one UTXO)
    let bob_coins = multi_wallet.all_coins_of(Address::Bob).unwrap();
    assert_eq!(bob_coins[0].1, 1700);
    assert_eq!(multi_wallet.total_assets_of(Address::Bob).unwrap(), 1700);
    // Charlie = 0
    let charlie_coins = multi_wallet.all_coins_of(Address::Charlie).unwrap();
    assert!(charlie_coins.is_empty());
    assert_eq!(multi_wallet.total_assets_of(Address::Charlie).unwrap(), 0);
    // Custom(100) = 0 (burned his coins)
    let custom_address_coins = multi_wallet.all_coins_of(Address::Custom(100)).unwrap();
    assert!(custom_address_coins.is_empty());
    assert_eq!(
        multi_wallet.total_assets_of(Address::Custom(100)).unwrap(),
        0
    );
    // Eve = 1300 (we don't own Eve yet, so we can't check her balance)
    assert_eq!(
        multi_wallet.total_assets_of(Address::Eve),
        Err(WalletError::ForeignAddress)
    );

    // Now, we add Eve to our owned addresses and check if we can now get her correct state
    // Since Eve was created in the first block after genesis, our restore_height will be 1
    multi_wallet.add_owned_address(Address::Eve, &node, 1);

    // Now we should be able to get Eve's balance and details!
    // Eve = 1300 (one 16 UTXO, one 50 UTXO and one 1234 UTXO)
    let eve_coins: Vec<(CoinId, u64)> = multi_wallet.all_coins_of(Address::Eve).unwrap();
    assert_eq!(eve_coins[0].1, 16);
    assert_eq!(eve_coins[1].1, 50);
    assert_eq!(eve_coins[2].1, 1234);
    assert_eq!(multi_wallet.total_assets_of(Address::Eve).unwrap(), 1300);

    // Since the address is owned, we should be able to create transactions with it:
    assert_eq!(
        multi_wallet.create_manual_transaction(
            vec![eve_coins[0].0],
            vec![Coin {
                owner: Address::Alice,
                value: 10
            }]
        ),
        Ok(Transaction {
            inputs: vec![Input {
                coin_id: eve_coins[0].0,
                signature: Signature::Valid(Address::Eve)
            }],
            outputs: vec![Coin {
                owner: Address::Alice,
                value: 10
            }]
        })
    );

    // And its net worth should be counted towards the total net worth of the wallet,
    // we test this by creating an automatic transaction that would spend more than the total
    // net worth without Eve, but with Eve it has enough money
    assert_eq!(
        multi_wallet.create_automatic_transaction(Address::Alice, 2900, 40),
        Ok(Transaction {
            inputs: vec![
                Input {
                    coin_id: eve_coins[0].0,
                    signature: Signature::Valid(Address::Eve)
                },
                Input {
                    coin_id: eve_coins[1].0,
                    signature: Signature::Valid(Address::Eve)
                },
                Input {
                    coin_id: eve_coins[2].0,
                    signature: Signature::Valid(Address::Eve)
                },
                Input {
                    coin_id: bob_coins[0].0,
                    signature: Signature::Valid(Address::Bob)
                }
            ],
            outputs: vec![
                Coin {
                    owner: Address::Alice,
                    value: 2900
                },
                Coin {
                    owner: Address::Bob,
                    value: 60
                }
            ]
        })
    ); // The error code is not the nicest but we can't change that
}

/// This next test advances the chain up, including transactions in each block such as minting,
/// transfering with tip, transfering without tip and burning. Then, it test the functionality
/// of adding a new watch-only address and making sure no transactions can be done using it
#[test]
fn add_watch_only_address_test() {
    // Create node and wallet
    let mut node = MockNode::new();

    // Create wallet with Alice
    let mut alice_wallet = wallet_with_alice();
    // Create a wallet with Bob, Charlie and Custom(100)
    let mut multi_wallet =
        wallet_with_addresses(vec![Address::Bob, Address::Charlie, Address::Custom(100)]);

    // Sync a chain to height 5, minting and transacting in the process
    let mint_tx = multiple_mint(
        vec![
            Address::Alice,
            Address::Bob,
            Address::Charlie,
            Address::Custom(100),
            Address::Eve,
        ],
        vec![100, 2000, 300, 50, 1234],
    );
    let alice_coins = mint_tx.coin_id(0);
    let bob_coins = mint_tx.coin_id(1);
    let charlie_coins = mint_tx.coin_id(2);
    let custom_address_coins = mint_tx.coin_id(3);

    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    let b2_id = node.add_block_as_best(
        b1_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: alice_coins,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![Coin {
                owner: Address::Eve,
                value: 50,
            }],
        }],
    );
    let b3_id = node.add_block_as_best(
        b2_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: charlie_coins,
                signature: Signature::Valid(Address::Charlie),
            }],
            outputs: vec![Coin {
                owner: Address::Alice,
                value: 150,
            }],
        }],
    );
    let b4_id = node.add_block_as_best(
        b3_id,
        vec![
            Transaction {
                inputs: vec![Input {
                    coin_id: bob_coins,
                    signature: Signature::Valid(Address::Bob),
                }],
                outputs: vec![
                    Coin {
                        owner: Address::Alice,
                        value: 300,
                    },
                    Coin {
                        owner: Address::Bob,
                        value: 1700,
                    },
                ],
            },
            Transaction {
                inputs: vec![],
                outputs: vec![Coin {
                    owner: Address::Eve,
                    value: 16,
                }],
            },
        ],
    );
    let _b5_id = node.add_block_as_best(
        b4_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: custom_address_coins,
                signature: Signature::Valid(Address::Custom(100)),
            }],
            outputs: vec![],
        }],
    );
    alice_wallet.sync(&node);
    multi_wallet.sync(&node);

    // Eve = 1300 (we don't own Eve yet, so we can't check her balance)
    assert_eq!(
        multi_wallet.total_assets_of(Address::Eve),
        Err(WalletError::ForeignAddress)
    );

    // Now, we add Eve to our owned addresses and check if we can now get her correct state
    // Since Eve was created in the first block after genesis, our restore_height will be 1
    multi_wallet.add_watch_only_address(Address::Eve, &node, 1);

    // Now we should be able to get Eve's balance and details!
    // Eve = 1300 (one 16 UTXO, one 50 UTXO and one 1234 UTXO)
    let eve_coins: Vec<(CoinId, u64)> = multi_wallet.all_coins_of(Address::Eve).unwrap();
    assert_eq!(eve_coins[0].1, 16);
    assert_eq!(eve_coins[1].1, 50);
    assert_eq!(eve_coins[2].1, 1234);
    assert_eq!(multi_wallet.total_assets_of(Address::Eve).unwrap(), 1300);

    // But, since the address is watch-only, we won't be able to use its coins to create transactions:
    assert_eq!(
        multi_wallet.create_manual_transaction(
            vec![eve_coins[0].0],
            vec![Coin {
                owner: Address::Alice,
                value: 10
            }]
        ),
        Err(WalletError::ForeignAddress)
    ); // The error code is not the nicest but we can't change that

    // Here, we try to create an automatic transaction with a value that's greater than the net worth
    // of our *owned* addresses, but smaller than the net worth of *all* our addresses. It should fail
    assert_eq!(
        multi_wallet.create_automatic_transaction(Address::Alice, 1800, 0),
        Err(WalletError::OutputsExceedInputs)
    )
}

/// This next test advances the chain up, including transactions in each block such as minting,
/// transfering with tip, transfering without tip and burning. Then, it tests the functionality
/// of removing a previously owned address and making sure no transactions can be done using it
#[test]
fn remove_address_test() {
    // Create node
    let mut node = MockNode::new();

    // Create wallet with Alice
    let mut alice_wallet = wallet_with_alice();
    // Create a wallet with Bob, Charlie and Custom(100)
    let mut multi_wallet =
        wallet_with_addresses(vec![Address::Bob, Address::Charlie, Address::Custom(100)]);

    // Sync a chain to height 5, minting and transacting in the process
    let mint_tx = multiple_mint(
        vec![
            Address::Alice,
            Address::Bob,
            Address::Charlie,
            Address::Custom(100),
            Address::Eve,
        ],
        vec![100, 2000, 300, 50, 1234],
    );
    let alice_coins = mint_tx.coin_id(0);
    let bob_coins = mint_tx.coin_id(1);
    let charlie_coins = mint_tx.coin_id(2);
    let custom_address_coins = mint_tx.coin_id(3);

    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    let b2_id = node.add_block_as_best(
        b1_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: alice_coins,
                signature: Signature::Valid(Address::Alice),
            }],
            outputs: vec![Coin {
                owner: Address::Eve,
                value: 50,
            }],
        }],
    );
    let b3_id = node.add_block_as_best(
        b2_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: charlie_coins,
                signature: Signature::Valid(Address::Charlie),
            }],
            outputs: vec![Coin {
                owner: Address::Alice,
                value: 150,
            }],
        }],
    );
    let b4_id = node.add_block_as_best(
        b3_id,
        vec![
            Transaction {
                inputs: vec![Input {
                    coin_id: bob_coins,
                    signature: Signature::Valid(Address::Bob),
                }],
                outputs: vec![
                    Coin {
                        owner: Address::Alice,
                        value: 300,
                    },
                    Coin {
                        owner: Address::Bob,
                        value: 1700,
                    },
                ],
            },
            Transaction {
                inputs: vec![],
                outputs: vec![Coin {
                    owner: Address::Eve,
                    value: 16,
                }],
            },
        ],
    );
    let _b5_id = node.add_block_as_best(
        b4_id,
        vec![Transaction {
            inputs: vec![Input {
                coin_id: custom_address_coins,
                signature: Signature::Valid(Address::Custom(100)),
            }],
            outputs: vec![],
        }],
    );
    alice_wallet.sync(&node);
    multi_wallet.sync(&node);

    // Eve = 1300 (we don't own Eve, so we can't check her balance)
    assert_eq!(
        multi_wallet.total_assets_of(Address::Eve),
        Err(WalletError::ForeignAddress)
    );

    // Now, we remove Bob from our owned addresses and check that we do not have access to his
    // information anymore
    assert_eq!(multi_wallet.remove_address(Address::Bob), Ok(()));

    // Now we should not be able to get Bob's balance and details!
    // Bob = 1700 (one 1700 UTXO)
    assert_eq!(
        multi_wallet.all_coins_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
    assert_eq!(
        multi_wallet.total_assets_of(Address::Bob),
        Err(WalletError::ForeignAddress)
    );
    // And since Bob was the only one with coins in the wallet, the net worth should now be 0
    assert_eq!(multi_wallet.net_worth(), 0);
}

/// This next test make sure that removing an address not owned nor watched by a wallet return error
#[test]
fn remove_not_existent_address() {
    let mut multi_wallet =
        wallet_with_addresses(vec![Address::Bob, Address::Charlie, Address::Custom(100)]);

    assert_eq!(
        multi_wallet.remove_address(Address::Alice),
        Err(WalletError::ForeignAddress)
    );
}

/// This next test make sure that adding an address owned or watched does nothing, and that adding
/// an already owned address as a watch only address also does nothing (and keeps the address as owned)
#[test]
fn adding_existing_address() {
    // Create a node and a wallet
    let mut node = MockNode::new();
    let mut multi_wallet =
        wallet_with_addresses(vec![Address::Bob, Address::Charlie, Address::Custom(100)]);

    // Sync a chain to height 1, minting coins in the process
    let mint_tx = multiple_mint(
        vec![
            Address::Alice,
            Address::Bob,
            Address::Charlie,
            Address::Custom(100),
            Address::Eve,
        ],
        vec![100, 2000, 300, 50, 1234],
    );
    node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    // Sync the wallet to get these minted coins
    multi_wallet.sync(&node);

    assert_eq!(multi_wallet.net_worth(), 2350);

    // Then we "add" Bob again to the wallet
    multi_wallet.add_owned_address(Address::Bob, &node, 0);
    // And nothing should have changed (we could check this more thoroughly but I'm lacking time :( )
    assert_eq!(multi_wallet.net_worth(), 2350);

    // We try again but now as a watch-only address. If now Bob is watch only, the networth should have diminished in 2000
    multi_wallet.add_watch_only_address(Address::Bob, &node, 0);
    assert_eq!(multi_wallet.net_worth(), 2350); // But it hasn't changed, which means Bob is still an owned address
}

/// This next test checks that the function to unify all UTXOs of a user into a single one works correctly
#[test]
fn test_unifying_utxos() {
    // Create a node and a wallet
    let mut node = MockNode::new();
    let mut alice_wallet = wallet_with_alice();

    // Sync a chain to height 1, minting coins in the process
    let mint_tx = multiple_mint(
        vec![Address::Alice, Address::Alice, Address::Alice],
        vec![100, 2000, 300],
    );
    let alice_coins = vec![
        (mint_tx.coin_id(0), 100),
        (mint_tx.coin_id(1), 2000),
        (mint_tx.coin_id(2), 300),
    ];
    node.add_block_as_best(Block::genesis().id(), vec![mint_tx]);
    // Sync the wallet to get these minted coins
    alice_wallet.sync(&node);

    // Check that Alice now has this newly minted coins
    assert_eq!(
        alice_wallet.all_coins_of(Address::Alice),
        Ok(alice_coins.clone())
    );

    assert_eq!(
        alice_wallet.unify_address_utxos(Address::Alice),
        Ok(Transaction {
            inputs: vec![
                Input {
                    coin_id: alice_coins[0].0,
                    signature: Signature::Valid(Address::Alice)
                },
                Input {
                    coin_id: alice_coins[1].0,
                    signature: Signature::Valid(Address::Alice)
                },
                Input {
                    coin_id: alice_coins[2].0,
                    signature: Signature::Valid(Address::Alice)
                }
            ],
            outputs: vec![Coin {
                value: 2400,
                owner: Address::Alice
            }]
        })
    );
}

/// This next test makes sure that trying to access any function that needs an address on an empty wallet errors out
#[test]
fn empty_wallet_errors() {
    let empty_wallet = wallet_with_addresses(vec![]);

    assert_eq!(
        empty_wallet.total_assets_of(Address::Alice),
        Err(WalletError::NoOwnedAddresses)
    );

    assert_eq!(empty_wallet.net_worth(), 0);

    assert_eq!(
        empty_wallet.all_coins_of(Address::Alice),
        Err(WalletError::NoOwnedAddresses)
    );

    assert_eq!(
        empty_wallet.create_manual_transaction(vec![], vec![]),
        Err(WalletError::NoOwnedAddresses)
    );

    assert_eq!(
        empty_wallet.create_automatic_transaction(Address::Alice, 1000, 100),
        Err(WalletError::NoOwnedAddresses)
    );
}

/// This next test makes sure that trying to get information of an unknown coin returns the correct error
/// and also trying to create a transaction with an unknown coin as well
#[test]
fn unknown_coin_test() {
    let multi_wallet = wallet_with_addresses(vec![Address::Alice, Address::Bob]);

    assert_eq!(
        multi_wallet.coin_details(&Input::dummy().coin_id),
        Err(WalletError::UnknownCoin)
    );

    assert_eq!(
        multi_wallet.create_manual_transaction(vec![Input::dummy().coin_id], vec![]),
        Err(WalletError::UnknownCoin)
    );
}
