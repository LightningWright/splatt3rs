use algorithm::Op;

fn rle_pack(instrs: &[Op]) -> Vec<(Op, u8)> {
    let mut out = Vec::new();
    let mut iter = instrs.iter();

    let Some(mut cur) = iter.next() else {
        return out;
    };

    let mut count: u8 = 1;

    for op in iter {
        if op == cur && count < 64 {
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

// 4+6 bit packing
pub fn rle_encode(instrs: &[Op]) -> Vec<u8> {
    let rle = rle_pack(instrs);

    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits: u8 = 0;

    let mut push = |value: u32, nbits: u8| {
        acc |= value << bits;
        bits += nbits;

        while bits >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            bits -= 8;
        }
    };

    for (op, len) in rle {
        push(op as u32, 4);
        push((len - 1) as u32, 6);
    }

    if bits > 0 {
        out.push(acc as u8);
    }

    out
}
