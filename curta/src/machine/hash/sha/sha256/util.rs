pub struct SHA256Util;

impl SHA256Util {
    pub fn pad(msg: &[u8]) -> Vec<u8> {
        let mut padded_msg = Vec::new();
        padded_msg.extend_from_slice(msg);
        padded_msg.push(1 << 7);

        // Find number of zeros
        let mdi = msg.len() % 64;
        assert!(mdi < 120);
        let padlen = if mdi < 56 { 55 - mdi } else { 119 - mdi };
        // Pad with zeros
        padded_msg.extend_from_slice(&vec![0u8; padlen]);

        // add length as 64 bit number
        let len = ((msg.len() * 8) as u64).to_be_bytes();
        padded_msg.extend_from_slice(&len);

        padded_msg
    }

    pub fn pre_process(chunk: &[u8]) -> [u32; 64] {
        let chunk_u32 = chunk
            .chunks_exact(4)
            .map(|x| u32::from_be_bytes(x.try_into().unwrap()))
            .collect::<Vec<_>>();
        let mut w = [0u32; 64];

        w[..16].copy_from_slice(&chunk_u32[..16]);

        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        w
    }

    pub fn process(hash: [u32; 8], w: &[u32; 64], round_constants: [u32; 64]) -> [u32; 8] {
        let mut msg = hash;
        for i in 0..64 {
            msg = Self::step(msg, w[i], round_constants[i]);
        }

        [
            hash[0].wrapping_add(msg[0]),
            hash[1].wrapping_add(msg[1]),
            hash[2].wrapping_add(msg[2]),
            hash[3].wrapping_add(msg[3]),
            hash[4].wrapping_add(msg[4]),
            hash[5].wrapping_add(msg[5]),
            hash[6].wrapping_add(msg[6]),
            hash[7].wrapping_add(msg[7]),
        ]
    }

    pub fn step(msg: [u32; 8], w_i: u32, round_constant: u32) -> [u32; 8] {
        let mut a = msg[0];
        let mut b = msg[1];
        let mut c = msg[2];
        let mut d = msg[3];
        let mut e = msg[4];
        let mut f = msg[5];
        let mut g = msg[6];
        let mut h = msg[7];

        let sum_1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let temp_1 = h
            .wrapping_add(sum_1)
            .wrapping_add(ch)
            .wrapping_add(round_constant)
            .wrapping_add(w_i);
        let sum_0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp_2 = sum_0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp_1);
        d = c;
        c = b;
        b = a;
        a = temp_1.wrapping_add(temp_2);

        [a, b, c, d, e, f, g, h]
    }

    pub fn decode(digest: &str) -> [u32; 8] {
        hex::decode(digest)
            .unwrap()
            .chunks_exact(4)
            .map(|x| u32::from_be_bytes(x.try_into().unwrap()))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }
}
