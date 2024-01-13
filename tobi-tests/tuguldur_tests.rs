use super::*;

// Helper Methods
fn empty_wallet() -> Wallet {
    Wallet::new(vec![].into_iter())
}

fn wallet_with_alice() -> Wallet {
    Wallet::new(vec![Address::Alice].into_iter())
}

fn wallet_with_alice_and_bob() -> Wallet {
    Wallet::new(vec![Address::Alice, Address::Bob].into_iter())
}

fn marker_tx() -> Transaction {
    Transaction {
        inputs: vec![Input::dummy()],
        outputs: vec![Coin {
            value: 123,
            owner: Address::Custom(123),
        }],
    }
}

// Official tests included in the starter repo
// This will check the basic values after wallet creation.
#[test]
fn correct_genesis_values() {
    let wallet = wallet_with_alice();

    assert_eq!(wallet.best_height(), 0);
    assert_eq!(wallet.best_hash(), Block::genesis().id());
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(0));
    assert_eq!(wallet.net_worth(), 0);
    assert_eq!(wallet.all_coins_of(Address::Alice).unwrap().len(), 0);
}

// This case ensures that the wallet will reject requests for foreign addresses.
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

// Check validation for an empty wallet with no registered addresses
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

// Empty transactions should be fine
#[test]
fn empty_manual_transaction() {
    let wallet = wallet_with_alice();

    let result = wallet.create_automatic_transaction(Address::Eve, 0, 0);
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

// Check the basic case of sync, should have correct height
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

// Should reorg to longer chain and calculate correct height
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

//                      Old_B2 (discard)   -     Old_B3 (discard)
//                  /
//              G
//                  \   B2      (should reorg the chain here)
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

// Check that sync updates with internal state of the wallet with transaction UTXO
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

    // Check that the coins have been correctly accounted for
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.net_worth(), COIN_VALUE);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(vec![(coin_id, COIN_VALUE)])
    );
    assert_eq!(wallet.coin_details(&coin_id), Ok(coin));
}

// Check that user can transfer to themselves
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

// Check that transaction between users correctly updates wallet state
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
        value: COIN_VALUE * 2,
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

    // Check that the coins have been correctly accounted for
    assert_eq!(wallet.total_assets_of(Address::Alice), Ok(COIN_VALUE));
    assert_eq!(wallet.total_assets_of(Address::Bob), Ok(COIN_VALUE * 2));
    assert_eq!(wallet.net_worth(), COIN_VALUE * 3);
    assert_eq!(
        wallet.all_coins_of(Address::Alice),
        Ok(vec![(coin_id1, COIN_VALUE)])
    );
    assert_eq!(
        wallet.all_coins_of(Address::Bob),
        Ok(vec![(coin_id2, COIN_VALUE * 2)])
    );
    assert_eq!(wallet.coin_details(&coin_id1), Ok(coin1));
    assert_eq!(wallet.coin_details(&coin_id2), Ok(coin2));
}

// Should be able to create a manual transaction by providing inputs and outputs
#[test]
fn manual_tx_should_succeed() {
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

// Should return an error when unknown coin is provided to create a manual transaction
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

// Should return an error if the output value is greater than input value
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

// Should be able to create an automatic transaction
#[test]
fn automatic_tx_should_succeed_zero_tip() {
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

// Should be able to create an automatic transaction with a tip
#[test]
fn automatic_tx_should_succeed_with_tip() {
    const COIN_VALUE: u64 = 100;
    let coin = Coin {
        value: COIN_VALUE * 2,
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
    let tx_result = wallet.create_automatic_transaction(Address::Charlie, COIN_VALUE, 100);
    assert!(tx_result.is_ok());
    let results = tx_result.unwrap();
    assert_eq!(results.outputs, output_coins);
    assert_eq!(results.inputs[0].coin_id, coin_id);
    assert_eq!(
        results.inputs[0].signature,
        Signature::Valid(Address::Alice)
    );
}

// Should be able to validate when output value exceeds available inputs
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

    // Check that the calculation is right
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

    // Check that the coins have been correctly accounted for
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

    // Check that the coins have been correctly accounted for
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

    // Check that the coins have been correctly accounted for
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

    // Check that the coins have been correctly accounted for
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
    let regular_sync_calls = node.how_many_queries();

    // Reorg to longer chain of same length
    let b4_id = node.add_block_as_best(old_b3_id, vec![]);
    node.add_block_as_best(b4_id, vec![]);
    wallet.sync(&node);
    let reorg_calls = node.how_many_queries() - regular_sync_calls;

    assert!(reorg_calls < regular_sync_calls);
}

// Check the reorg case until the genesis block
//          B2 (discard)  -  B3 (discard)
//        /
//    G
//        \
//          C2            -  C3             -       C4          -        C5 (new wallet state)
#[test]
fn deep_reorg() {
    let mut node = MockNode::new();
    let mut wallet = wallet_with_alice();

    // Sync a chain to height 3
    let b1_id = node.add_block_as_best(Block::genesis().id(), vec![]);
    let b2_id = node.add_block_as_best(b1_id, vec![]);
    node.add_block_as_best(b2_id, vec![]);
    wallet.sync(&node);

    let c1_id = node.add_block(Block::genesis().id(), vec![marker_tx()]);
    let c2_id = node.add_block_as_best(c1_id, vec![]);
    let c3_id = node.add_block_as_best(c2_id, vec![]);
    let c4_id = node.add_block_as_best(c3_id, vec![]);
    let c5_id = node.add_block_as_best(c4_id, vec![]);
    wallet.sync(&node);

    assert_eq!(wallet.best_height(), 5);
    assert_eq!(wallet.best_hash(), c5_id);
}
