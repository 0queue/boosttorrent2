use crypto::{
    digest::Digest,
    sha1::Sha1
};
use bit_vec::BitVec;

/// Holds the data of a downloaded piece
pub struct Piece {
    data: Vec<u8>,
    hasher: Sha1,
    hash: [u8; 20],
    // Pieces can be arbitrarily sized, but requests can be no larger than 16k.  This keeps track
    // of which pieces of the larger piece we have collected
    sub_pieces: BitVec
}

impl Piece {
    pub fn new(piece_size: u32, piece_hash: [u8;20]) -> Self {
        let mut num_subpieces = piece_size / (1 << 14);
        num_subpieces += if piece_size % (1 << 14) == 0 { 0 } else { 1 };
        Piece {
            data: Vec::with_capacity(piece_size as usize),
            hasher: Sha1::new(),
            hash: piece_hash,
            sub_pieces: BitVec::from_elem(num_subpieces as usize, false)
        }
    }

    pub fn verify(&mut self) -> bool {
        self.hasher.reset();
        self.hasher.input(&self.data);
        let mut data_hash: [u8;20] = [0;20];
        self.hasher.result(&mut data_hash);
        return data_hash == self.hash;
    }

}