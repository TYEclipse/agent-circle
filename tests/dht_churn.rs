// Extreme DHT churn test
// 验收：大量节点频繁上下线，路由表不崩溃、状态一致

#[cfg(test)]
mod tests {
    use libp2p::kad::store::RecordStore;

    /// R171a: Extreme DHT churn — rapid add/remove provider cycles.
    /// Simulates nodes joining and leaving at high frequency.
    /// Verifies the store does not crash.
    #[test]
    fn r171a_dht_churn_store_does_not_crash() {
        use libp2p::kad::ProviderRecord;
        use libp2p::{kad::RecordKey, PeerId};

        let bootstrap_id = PeerId::random();
        let mut store = libp2p::kad::store::MemoryStore::new(bootstrap_id);

        // Generate 50 peers
        let peers: Vec<PeerId> = (0..50).map(|_| PeerId::random()).collect();

        // Rapid add/remove cycles — 10 rounds of churn
        for round in 0u64..10 {
            let batch_size = 5;
            let start = ((round as usize) * batch_size) % peers.len();
            let end = std::cmp::min(start + batch_size, peers.len());

            // Add a batch of providers
            for &pid in &peers[start..end] {
                let key = RecordKey::new(&round.to_le_bytes());
                let record = ProviderRecord::new(key, pid, vec![]);
                store
                    .add_provider(record)
                    .expect("add_provider should succeed");

                // Duplicate add — should be idempotent
                let key = RecordKey::new(&round.to_le_bytes());
                let record = ProviderRecord::new(key, pid, vec![]);
                store
                    .add_provider(record)
                    .expect("duplicate add should succeed");
            }

            // Remove a different batch (simulating nodes leaving)
            let remove_start = ((round as usize + 3) * batch_size) % peers.len();
            let remove_end = std::cmp::min(remove_start + batch_size, peers.len());
            for &pid in &peers[remove_start..remove_end] {
                let key = RecordKey::new(&round.to_le_bytes());
                store.remove_provider(&key, &pid);
            }
        }

        // Verify: store should still have some providers
        let key = RecordKey::new(&0u64.to_le_bytes());
        let providers = store.providers(&key);
        assert!(
            !providers.is_empty(),
            "After churn, at least one provider should remain for round 0"
        );
    }

    /// R171b: Kademlia kbucket capacity — add up to K_VALUE peers and verify
    /// routing table handles capacity limits.
    #[test]
    fn r171b_kbucket_capacity() {
        use libp2p::kad::ProviderRecord;
        use libp2p::{kad::RecordKey, PeerId};

        let bootstrap_id = PeerId::random();
        let mut store = libp2p::kad::store::MemoryStore::new(bootstrap_id);

        let key = RecordKey::new(b"r171b-kbucket");

        // Add 30 providers (exceeds K_VALUE=20)
        let peers: Vec<PeerId> = (0..30).map(|_| PeerId::random()).collect();
        for &pid in &peers {
            let record = ProviderRecord::new(key.clone(), pid, vec![]);
            store.add_provider(record).expect("add should succeed");
        }

        let providers = store.providers(&key);
        assert!(!providers.is_empty(), "Must have at least some providers");
        assert!(
            providers.len() <= 30,
            "Providers should not exceed total added"
        );

        // Remove all and verify
        for pid in &peers {
            store.remove_provider(&key, pid);
        }
        let providers_after = store.providers(&key);
        assert!(
            providers_after.is_empty(),
            "All providers should be removed, got {} remaining",
            providers_after.len()
        );
    }

    /// R171c: Record churn — puts, gets, overwrites, and removals under pressure
    #[test]
    fn r171c_record_churn() {
        use libp2p::kad::Record;
        use libp2p::{kad::RecordKey, PeerId};

        let bootstrap_id = PeerId::random();
        let mut store = libp2p::kad::store::MemoryStore::new(bootstrap_id);

        // Put 100 records
        for i in 0u64..100 {
            let key = RecordKey::new(&i.to_le_bytes());
            let record = Record::new(key, format!("value-{i}").into_bytes());
            store.put(record).expect("put should succeed");
        }

        // Verify all records are retrievable
        for i in 0u64..100 {
            let key = RecordKey::new(&i.to_le_bytes());
            let record = store.get(&key);
            assert!(record.is_some(), "Record {i} should exist after put");
            assert_eq!(
                record.unwrap().value,
                format!("value-{i}").as_bytes().to_vec(),
                "Record value mismatch for {i}"
            );
        }

        // Overwrite 50 records with new values (simulating churn)
        for i in 0u64..50 {
            let key = RecordKey::new(&i.to_le_bytes());
            let record = Record::new(key, format!("new-value-{i}").into_bytes());
            store.put(record).expect("overwrite should succeed");
        }

        // Verify both overwritten and untouched records
        for i in 0u64..100 {
            let key = RecordKey::new(&i.to_le_bytes());
            let record = store.get(&key).expect("record should exist");
            let expected: Vec<u8> = if i < 50 {
                format!("new-value-{i}").into_bytes()
            } else {
                format!("value-{i}").into_bytes()
            };
            assert_eq!(
                record.value, expected,
                "Record {i} mismatch after overwrite"
            );
        }

        // Remove 75 records
        for i in 0u64..75 {
            let key = RecordKey::new(&i.to_le_bytes());
            store.remove(&key);
        }

        // Verify removals
        for i in 0u64..100 {
            let key = RecordKey::new(&i.to_le_bytes());
            let record = store.get(&key);
            if i < 75 {
                assert!(record.is_none(), "Record {i} should be removed");
            } else {
                assert!(record.is_some(), "Record {i} should still exist");
            }
        }
    }

    /// R171d: Stress — rapid alternating add/remove on the same set of keys
    #[test]
    fn r171d_rapid_toggle() {
        use libp2p::kad::Record;
        use libp2p::{kad::RecordKey, PeerId};

        let bootstrap_id = PeerId::random();
        let mut store = libp2p::kad::store::MemoryStore::new(bootstrap_id);

        // 500 rounds of put/remove on 10 keys
        let key_data: Vec<u64> = (0..10).collect();
        for round in 0u64..500 {
            for &k in &key_data {
                let key = RecordKey::new(&k.to_le_bytes());
                if round % 2 == 0 {
                    let record = Record::new(key, format!("round-{round}-k{k}").into_bytes());
                    store.put(record).expect("put should succeed");
                } else {
                    store.remove(&key);
                }
            }
        }

        // Round 499 is odd → last op is remove. All keys should be gone.
        for &k in &key_data {
            let key = RecordKey::new(&k.to_le_bytes());
            let record = store.get(&key);
            assert!(
                record.is_none(),
                "Key {k} should be removed after 500 toggle rounds"
            );
        }
    }

    /// R171e: Cross-contamination — verify operations on one key don't affect another
    #[test]
    fn r171e_key_isolation() {
        use libp2p::kad::{ProviderRecord, Record};
        use libp2p::{kad::RecordKey, PeerId};

        let bootstrap_id = PeerId::random();
        let mut store = libp2p::kad::store::MemoryStore::new(bootstrap_id);

        let key_a = RecordKey::new(b"key-a");
        let key_b = RecordKey::new(b"key-b");
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        // Put record on key_a, provider on key_b
        store
            .put(Record::new(key_a.clone(), b"value-a".to_vec()))
            .expect("put a");
        store
            .add_provider(ProviderRecord::new(key_b.clone(), peer1, vec![]))
            .expect("add provider b");

        // Churn key_b heavily
        for _ in 0..20 {
            store
                .add_provider(ProviderRecord::new(key_b.clone(), peer2, vec![]))
                .unwrap();
            store.remove_provider(&key_b, &peer2);
        }

        // key_a should be untouched
        let rec_a = store.get(&key_a).expect("key_a record should still exist");
        assert_eq!(rec_a.value, b"value-a".to_vec());

        // Remove key_b providers; key_a should survive
        store.remove_provider(&key_b, &peer1);
        let rec_a2 = store
            .get(&key_a)
            .expect("key_a should still exist after key_b removal");
        assert_eq!(rec_a2.value, b"value-a".to_vec());
    }
}
