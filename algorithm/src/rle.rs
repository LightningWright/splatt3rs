use algorithm::Op;

fn rle_pack(instrs: &[Op], rle_bits: u64) -> Vec<(Op, u64)> {
    let rle_len = 1u64 << rle_bits;
    let mut out = Vec::new();
    let mut iter = instrs.iter();

    let Some(mut cur) = iter.next() else {
        return out;
    };

    let mut count: u64 = 1;

    for op in iter {
        if op == cur && count < rle_len {
            count += 1;
        } else {
            out.push((*cur, count));
            cur = op;
            count = 1;
        }
    }

    out.push((*cur, count));
    out
}

pub fn rle_encode(instrs: &[Op], op_bits: u8, rle_bits: u64) -> Vec<u8> {
    let rle = rle_pack(instrs, rle_bits);

    let mut out = Vec::new();
    let mut acc: u64 = 0;
    let mut bits: u8 = 0;

    let mut push = |value: u64, nbits: u8| {
        acc |= value << bits;
        bits += nbits;

        while bits >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            bits -= 8;
        }
    };

    for (op, len) in rle {
        push(op as u64, op_bits);
        push(len - 1, rle_bits as u8);
    }

    if bits > 0 {
        out.push((acc | (0xFFu64 << bits)) as u8);
    }

    out
}

// Search for the RLE width that minimizes the encoded output size.
pub fn rle_encode_optimal(instrs: &[Op], op_bits: u8, max_rle_bits: u64) -> (Vec<u8>, u64) {
    let mut best_encoded = Vec::new();
    let mut best_width = 0;
    let mut best_size = usize::MAX;

    for width in 0..=max_rle_bits {
        let encoded = rle_encode(instrs, op_bits, width);

        if encoded.len() < best_size {
            best_size = encoded.len();
            best_width = width;
            best_encoded = encoded;
        }
    }

    println!(
        "RLE Encoding @ (OP-LEN) (4-{best_width}) {} Instrs {} Bytes",
        instrs.len(),
        best_encoded.len()
    );
    (best_encoded, best_width)
}
