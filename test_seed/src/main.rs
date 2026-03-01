use fips204::ml_dsa_65;
use fips204::traits::{KeyGen, SerDes};
use rand_core::{RngCore, CryptoRng};

struct DetRng(Vec<u8>, usize);
impl CryptoRng for DetRng {}
impl RngCore for DetRng {
    fn next_u32(&mut self) -> u32 { 0 }
    fn next_u64(&mut self) -> u64 { 0 }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for b in dest.iter_mut() {
            *b = self.0[self.1 % self.0.len()];
            self.1 += 1;
        }
    }
}

fn main() {
    let mut rng = DetRng(vec![42; 32], 0);
    // Dilithium3 / ML-DSA-65
    let (pk, sk) = ml_dsa_65::try_keygen_with_rng(&mut rng).unwrap();
    println!("PK length: {}", pk.into_bytes().len());
    println!("SK length: {}", sk.into_bytes().len());
}
