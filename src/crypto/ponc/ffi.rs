#[cxx::bridge]
pub mod bridge {
    unsafe extern "C++" {
        include!("ponc.h");

        type PoncEngine;

        fn new_ponc_engine() -> UniquePtr<PoncEngine>;

        fn initialize_scratchpad(
            self: Pin<&mut PoncEngine>,
            prev_hash: &[u8],
            miner_address: &[u8],
        );

        fn compute_and_verify(
            self: &PoncEngine,
            header_prefix: &[u8],
            nonce: u64,
            target_bytes: &[u8],
            out_hash: &mut [u8],
        ) -> bool;

        fn set_rounds(self: Pin<&mut PoncEngine>, rounds: usize);
    }
}

#[cfg(test)]
mod tests {
    use super::bridge::new_ponc_engine;

    #[test]
    fn test_ponc_basic() {
        let mut engine = new_ponc_engine();
        engine
            .pin_mut()
            .initialize_scratchpad(&[0u8; 32], &[1u8; 32]);

        let mut out = [0u8; 32];
        assert!(engine.compute_and_verify(&[0u8; 140], 0, &[0xFF; 32], &mut out));
        assert!(!engine.compute_and_verify(&[0u8; 140], 0, &[0x00; 32], &mut out));
    }
}
