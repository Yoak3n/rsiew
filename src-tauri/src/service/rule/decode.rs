
pub fn decode_mozlz4_bytes(data: &[u8]) -> std::result::Result<Vec<u8>, String> {
    const HEADER: &[u8; 8] = b"mozLz40\0";

    if data.len() < 12 {
        return Err("mozlz4 数据长度不足".to_string());
    }
    if &data[..8] != HEADER {
        return Err("mozlz4 文件头不匹配".to_string());
    }

    let expected_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let src = &data[12..];
    let mut out = Vec::with_capacity(expected_len);
    let mut index = 0usize;

    while index < src.len() {
        let token = src[index];
        index += 1;

        let mut literal_len = (token >> 4) as usize;
        if literal_len == 15 {
            loop {
                let extra = *src
                    .get(index)
                    .ok_or_else(|| "mozlz4 字面量长度越界".to_string())?;
                index += 1;
                literal_len += extra as usize;
                if extra != 255 {
                    break;
                }
            }
        }

        let literal_end = index + literal_len;
        if literal_end > src.len() {
            return Err("mozlz4 字面量块越界".to_string());
        }
        out.extend_from_slice(&src[index..literal_end]);
        index = literal_end;

        if index >= src.len() {
            break;
        }

        let offset = u16::from_le_bytes([
            *src.get(index)
                .ok_or_else(|| "mozlz4 offset 越界".to_string())?,
            *src.get(index + 1)
                .ok_or_else(|| "mozlz4 offset 越界".to_string())?,
        ]) as usize;
        index += 2;

        if offset == 0 || offset > out.len() {
            return Err("mozlz4 offset 非法".to_string());
        }

        let mut match_len = (token & 0x0F) as usize;
        if match_len == 15 {
            loop {
                let extra = *src
                    .get(index)
                    .ok_or_else(|| "mozlz4 匹配长度越界".to_string())?;
                index += 1;
                match_len += extra as usize;
                if extra != 255 {
                    break;
                }
            }
        }
        match_len += 4;

        let mut match_index = out.len() - offset;
        for _ in 0..match_len {
            let value = *out
                .get(match_index)
                .ok_or_else(|| "mozlz4 匹配引用越界".to_string())?;
            out.push(value);
            match_index += 1;
        }
    }

    if out.len() != expected_len {
        return Err(format!(
            "mozlz4 解码长度不匹配: expected={}, actual={}",
            expected_len,
            out.len()
        ));
    }

    Ok(out)
}