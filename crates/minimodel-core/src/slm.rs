use std::collections::HashSet;

const HEADER_LEN: usize = 108;
const TENSOR_ENTRY_LEN: usize = 64;
const SLM_VERSION: u32 = 1;
const DTYPE_F32: u32 = 1;
const DTYPE_Q8_0: u32 = 2;
const DTYPE_Q4_0: u32 = 3;

#[derive(Clone, Copy, Debug)]
struct SlmHeader {
    version: u32,
    header_length: u32,
    model_type: u32,
    flags: u32,
    vocab_size: u32,
    special_token_count: u32,
    hidden_size: u32,
    layer_count: u32,
    head_count: u32,
    kv_head_count: u32,
    head_dim: u32,
    ffn_size: u32,
    max_context: u32,
    rope_theta: f32,
    rms_norm_epsilon: f32,
    tokenizer_offset: u64,
    tokenizer_length: u64,
    tensor_directory_offset: u64,
    tensor_count: u32,
    tensor_data_offset: u64,
    checksum: u64,
}

#[derive(Clone, Copy, Debug)]
struct TensorEntry {
    name_hash: u64,
    dtype: u32,
    rank: u32,
    dims: [u32; 4],
    byte_offset: u64,
    byte_length: u64,
    scale_offset: u64,
    block_size: u32,
}

#[derive(Debug, Clone)]
pub struct SlmReport {
    pub file_bytes: usize,
    pub format_version: u32,
    pub model_shape: String,
    pub quantization: String,
    pub checksum: String,
    pub tokenizer_checksum: String,
    pub tensor_layout_checksum: String,
}

pub fn validate_slm_bytes(bytes: &[u8]) -> Result<SlmReport, String> {
    let header = parse_header(bytes)?;
    validate_header(bytes, &header)?;
    validate_checksum(bytes, &header)?;
    validate_tokenizer(bytes, &header)?;
    let tensors = parse_tensors(bytes, &header)?;
    validate_unique_tensors(&tensors)?;
    validate_required_tensors(&header, &tensors)?;
    validate_tensor_payloads(bytes, &tensors)?;
    Ok(SlmReport {
        file_bytes: bytes.len(),
        format_version: header.version,
        model_shape: shape_summary(&header),
        quantization: quantization_label(&tensors).to_string(),
        checksum: format!("0x{:016x}", header.checksum),
        tokenizer_checksum: format!("0x{:016x}", tokenizer_checksum(bytes, &header)?),
        tensor_layout_checksum: format!("0x{:016x}", tensor_layout_checksum(&tensors)),
    })
}

fn parse_header(bytes: &[u8]) -> Result<SlmHeader, String> {
    if bytes.len() < HEADER_LEN {
        return Err("file is shorter than the v1 header".to_string());
    }
    if bytes.get(0..4) != Some(b"SLM1") {
        return Err("invalid SLM magic".to_string());
    }
    Ok(SlmHeader {
        version: read_u32(bytes, 4)?,
        header_length: read_u32(bytes, 8)?,
        model_type: read_u32(bytes, 12)?,
        flags: read_u32(bytes, 16)?,
        vocab_size: read_u32(bytes, 20)?,
        special_token_count: read_u32(bytes, 24)?,
        hidden_size: read_u32(bytes, 28)?,
        layer_count: read_u32(bytes, 32)?,
        head_count: read_u32(bytes, 36)?,
        kv_head_count: read_u32(bytes, 40)?,
        head_dim: read_u32(bytes, 44)?,
        ffn_size: read_u32(bytes, 48)?,
        max_context: read_u32(bytes, 52)?,
        rope_theta: f32::from_bits(read_u32(bytes, 56)?),
        rms_norm_epsilon: f32::from_bits(read_u32(bytes, 60)?),
        tokenizer_offset: read_u64(bytes, 64)?,
        tokenizer_length: read_u64(bytes, 72)?,
        tensor_directory_offset: read_u64(bytes, 80)?,
        tensor_count: read_u32(bytes, 88)?,
        tensor_data_offset: read_u64(bytes, 92)?,
        checksum: read_u64(bytes, 100)?,
    })
}

fn validate_header(bytes: &[u8], header: &SlmHeader) -> Result<(), String> {
    if header.version != SLM_VERSION {
        return Err("unsupported SLM version".to_string());
    }
    if header.header_length < HEADER_LEN as u32 || header.header_length as usize > bytes.len() {
        return Err("invalid header length".to_string());
    }
    if header.model_type != 1 {
        return Err("unsupported model type".to_string());
    }
    if header.vocab_size < 260 || header.special_token_count < 4 {
        return Err("byte tokenizer requires vocab >= 260 and four specials".to_string());
    }
    if header.hidden_size == 0
        || header.layer_count == 0
        || header.head_count == 0
        || header.kv_head_count == 0
        || header.head_dim == 0
        || header.ffn_size == 0
        || header.max_context == 0
    {
        return Err("model dimensions must be nonzero".to_string());
    }
    if header.head_count.checked_mul(header.head_dim) != Some(header.hidden_size) {
        return Err("hidden_size must equal head_count * head_dim".to_string());
    }
    if header.kv_head_count > header.head_count
        || !header.head_count.is_multiple_of(header.kv_head_count)
    {
        return Err("kv_head_count must divide head_count".to_string());
    }
    if !header.rope_theta.is_finite() || header.rope_theta <= 0.0 {
        return Err("rope_theta must be finite and positive".to_string());
    }
    if !header.rms_norm_epsilon.is_finite() || header.rms_norm_epsilon <= 0.0 {
        return Err("rms_norm_epsilon must be finite and positive".to_string());
    }
    if header.checksum == 0 {
        return Err("checksum must be nonzero".to_string());
    }
    if header.tokenizer_offset < u64::from(header.header_length) {
        return Err("tokenizer overlaps the header".to_string());
    }
    if !range_within(
        bytes.len(),
        header.tokenizer_offset,
        header.tokenizer_length,
    ) {
        return Err("tokenizer range escapes the file".to_string());
    }
    if !header.tensor_directory_offset.is_multiple_of(64)
        || !header.tensor_data_offset.is_multiple_of(64)
    {
        return Err("tensor directory and data offsets must be 64-byte aligned".to_string());
    }
    let directory_len = u64::from(header.tensor_count)
        .checked_mul(TENSOR_ENTRY_LEN as u64)
        .ok_or_else(|| "tensor directory length overflow".to_string())?;
    if !range_within(bytes.len(), header.tensor_directory_offset, directory_len) {
        return Err("tensor directory range escapes the file".to_string());
    }
    if header.tensor_data_offset < header.tensor_directory_offset + directory_len {
        return Err("tensor data begins before the tensor directory ends".to_string());
    }
    Ok(())
}

fn validate_checksum(bytes: &[u8], header: &SlmHeader) -> Result<(), String> {
    let actual = simple_checksum(bytes);
    if header.checksum != actual {
        return Err("checksum mismatch".to_string());
    }
    Ok(())
}

fn validate_tokenizer(bytes: &[u8], header: &SlmHeader) -> Result<(), String> {
    let start = as_usize(header.tokenizer_offset)?;
    let len = as_usize(header.tokenizer_length)?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "tokenizer section length overflow".to_string())?;
    let section = bytes
        .get(start..end)
        .ok_or_else(|| "tokenizer section escapes the file".to_string())?;
    match section.get(0..4) {
        Some(b"BTOK") => validate_byte_tokenizer(section),
        Some(b"BPE1") => validate_bpe_tokenizer(section, header.vocab_size),
        _ => Err("unsupported tokenizer section magic".to_string()),
    }
}

fn validate_byte_tokenizer(section: &[u8]) -> Result<(), String> {
    if section.len() < 28 {
        return Err("byte tokenizer section is too short".to_string());
    }
    if read_u32(section, 4)? != 1 {
        return Err("unsupported byte tokenizer section version".to_string());
    }
    if read_u32(section, 8)? != 260 {
        return Err("phase-one byte tokenizer must declare 260 tokens".to_string());
    }
    if read_u32(section, 12)? != 256
        || read_u32(section, 16)? != 257
        || read_u32(section, 20)? != 258
        || read_u32(section, 24)? != 259
    {
        return Err("phase-one byte tokenizer special ids are invalid".to_string());
    }
    Ok(())
}

fn validate_bpe_tokenizer(section: &[u8], header_vocab_size: u32) -> Result<(), String> {
    if section.len() < 36 {
        return Err("BPE tokenizer section is too short".to_string());
    }
    if read_u32(section, 4)? != 1 {
        return Err("unsupported BPE tokenizer section version".to_string());
    }
    let vocab_size = read_u32(section, 8)?;
    if vocab_size != header_vocab_size || vocab_size < 260 {
        return Err("BPE tokenizer vocab must match the model header".to_string());
    }
    for offset in [12usize, 16, 20, 24] {
        if read_u32(section, offset)? >= vocab_size {
            return Err("BPE tokenizer special id escapes vocabulary".to_string());
        }
    }
    let token_count = read_u32(section, 28)? as usize;
    let merge_count = read_u32(section, 32)? as usize;
    let mut offset = 36usize;
    let mut token_ids = HashSet::new();
    for _ in 0..token_count {
        let id = read_u32(section, offset)?;
        let len = read_u32(section, offset + 4)? as usize;
        offset = offset
            .checked_add(8)
            .ok_or_else(|| "BPE token table offset overflow".to_string())?;
        let end = offset
            .checked_add(len)
            .ok_or_else(|| "BPE token byte length overflow".to_string())?;
        let token_bytes = section
            .get(offset..end)
            .ok_or_else(|| "BPE token bytes escape tokenizer section".to_string())?;
        if id < 260 || id >= vocab_size || token_bytes.is_empty() {
            return Err("BPE token table entry is invalid".to_string());
        }
        if !token_ids.insert(id) {
            return Err("BPE token ids must be unique".to_string());
        }
        offset = end;
    }
    for _ in 0..merge_count {
        let end = offset
            .checked_add(16)
            .ok_or_else(|| "BPE merge table offset overflow".to_string())?;
        if end > section.len() {
            return Err("BPE merge table escapes tokenizer section".to_string());
        }
        let left = read_u32(section, offset)?;
        let right = read_u32(section, offset + 4)?;
        let output = read_u32(section, offset + 8)?;
        if left >= vocab_size || right >= vocab_size || output >= vocab_size || output < 260 {
            return Err("BPE merge ids must stay inside vocabulary".to_string());
        }
        if !token_ids.contains(&output) {
            return Err("BPE merge output must exist in token table".to_string());
        }
        offset = end;
    }
    if offset != section.len() {
        return Err("BPE tokenizer section has trailing bytes".to_string());
    }
    Ok(())
}

fn parse_tensors(bytes: &[u8], header: &SlmHeader) -> Result<Vec<TensorEntry>, String> {
    let start = as_usize(header.tensor_directory_offset)?;
    let mut tensors = Vec::with_capacity(header.tensor_count as usize);
    for index in 0..header.tensor_count as usize {
        let base = start
            .checked_add(index * TENSOR_ENTRY_LEN)
            .ok_or_else(|| "tensor directory offset overflow".to_string())?;
        let entry = TensorEntry {
            name_hash: read_u64(bytes, base)?,
            dtype: read_u32(bytes, base + 8)?,
            rank: read_u32(bytes, base + 12)?,
            dims: [
                read_u32(bytes, base + 16)?,
                read_u32(bytes, base + 20)?,
                read_u32(bytes, base + 24)?,
                read_u32(bytes, base + 28)?,
            ],
            byte_offset: read_u64(bytes, base + 32)?,
            byte_length: read_u64(bytes, base + 40)?,
            scale_offset: read_u64(bytes, base + 48)?,
            block_size: read_u32(bytes, base + 56)?,
        };
        validate_tensor_directory_entry(bytes.len(), &entry)?;
        tensors.push(entry);
    }
    Ok(tensors)
}

fn validate_tensor_directory_entry(file_len: usize, tensor: &TensorEntry) -> Result<(), String> {
    if tensor.rank == 0 || tensor.rank > 4 {
        return Err("tensor rank must be between one and four".to_string());
    }
    if !matches!(tensor.dtype, DTYPE_F32 | DTYPE_Q8_0 | DTYPE_Q4_0) {
        return Err("unsupported tensor dtype".to_string());
    }
    for dim in tensor.dims.iter().take(tensor.rank as usize) {
        if *dim == 0 {
            return Err("tensor dimensions must be nonzero".to_string());
        }
    }
    if !tensor.byte_offset.is_multiple_of(64) {
        return Err("tensor payload offset must be 64-byte aligned".to_string());
    }
    if !range_within(file_len, tensor.byte_offset, tensor.byte_length) {
        return Err("tensor payload escapes the file".to_string());
    }
    let expected = expected_payload_len(tensor)?;
    if tensor.byte_length != expected {
        return Err("tensor byte length does not match dtype and shape".to_string());
    }
    match tensor.dtype {
        DTYPE_F32 => {
            if tensor.scale_offset != 0 || tensor.block_size != 0 {
                return Err("f32 tensors must not declare quantization metadata".to_string());
            }
        }
        DTYPE_Q8_0 => {
            if tensor.scale_offset == 0 {
                return Err("q8_0 tensors require scale metadata".to_string());
            }
            if tensor.block_size != cols_per_row(tensor)? as u32 {
                return Err("q8_0 block_size must equal columns per row".to_string());
            }
            let scale_len = rows(tensor)?
                .checked_mul(4)
                .ok_or_else(|| "q8_0 scale length overflow".to_string())?;
            if !range_within(file_len, tensor.scale_offset, scale_len as u64) {
                return Err("q8_0 scale range escapes the file".to_string());
            }
        }
        DTYPE_Q4_0 => {
            if tensor.scale_offset == 0 || tensor.block_size == 0 {
                return Err("q4_0 tensors require scale metadata and block size".to_string());
            }
            let cols = cols_per_row(tensor)?;
            let block = tensor.block_size as usize;
            if !block.is_multiple_of(2) || !cols.is_multiple_of(block) {
                return Err("q4_0 block size must be even and divide columns per row".to_string());
            }
            let scale_count = rows(tensor)?
                .checked_mul(cols / block)
                .ok_or_else(|| "q4_0 scale count overflow".to_string())?;
            let scale_len = scale_count
                .checked_mul(4)
                .ok_or_else(|| "q4_0 scale length overflow".to_string())?;
            if !range_within(file_len, tensor.scale_offset, scale_len as u64) {
                return Err("q4_0 scale range escapes the file".to_string());
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_unique_tensors(tensors: &[TensorEntry]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for tensor in tensors {
        if !seen.insert(tensor.name_hash) {
            return Err("duplicate tensor name hash".to_string());
        }
    }
    Ok(())
}

fn validate_required_tensors(header: &SlmHeader, tensors: &[TensorEntry]) -> Result<(), String> {
    require_tensor(
        tensors,
        "tok_embeddings.weight",
        &[header.vocab_size, header.hidden_size],
    )?;
    require_tensor(tensors, "norm.weight", &[header.hidden_size])?;
    if header.flags & 1 == 0 {
        require_tensor(
            tensors,
            "output.weight",
            &[header.vocab_size, header.hidden_size],
        )?;
    }
    for layer in 0..header.layer_count {
        let prefix = format!("layers.{layer}");
        require_tensor(
            tensors,
            &format!("{prefix}.attention_norm.weight"),
            &[header.hidden_size],
        )?;
        require_tensor(
            tensors,
            &format!("{prefix}.ffn_norm.weight"),
            &[header.hidden_size],
        )?;
        for suffix in ["wq.weight", "wk.weight", "wv.weight", "wo.weight"] {
            require_tensor(
                tensors,
                &format!("{prefix}.{suffix}"),
                &[header.hidden_size, header.hidden_size],
            )?;
        }
        require_tensor(
            tensors,
            &format!("{prefix}.w1.weight"),
            &[header.ffn_size, header.hidden_size],
        )?;
        require_tensor(
            tensors,
            &format!("{prefix}.w2.weight"),
            &[header.hidden_size, header.ffn_size],
        )?;
        require_tensor(
            tensors,
            &format!("{prefix}.w3.weight"),
            &[header.ffn_size, header.hidden_size],
        )?;
    }
    Ok(())
}

fn require_tensor(
    tensors: &[TensorEntry],
    name: &str,
    expected_dims: &[u32],
) -> Result<(), String> {
    let hash = name_hash(name);
    let tensor = tensors
        .iter()
        .find(|entry| entry.name_hash == hash)
        .ok_or_else(|| format!("missing required tensor {name}"))?;
    if dims(tensor) != expected_dims {
        return Err(format!("shape mismatch for tensor {name}"));
    }
    Ok(())
}

fn validate_tensor_payloads(bytes: &[u8], tensors: &[TensorEntry]) -> Result<(), String> {
    for tensor in tensors {
        match tensor.dtype {
            DTYPE_F32 => validate_f32_payload(bytes, tensor)?,
            DTYPE_Q8_0 => validate_scales(bytes, tensor.scale_offset, rows(tensor)?)?,
            DTYPE_Q4_0 => {
                let block = tensor.block_size as usize;
                let scale_count = rows(tensor)?
                    .checked_mul(cols_per_row(tensor)? / block)
                    .ok_or_else(|| "q4_0 scale count overflow".to_string())?;
                validate_scales(bytes, tensor.scale_offset, scale_count)?;
            }
            _ => return Err("unsupported tensor dtype".to_string()),
        }
    }
    Ok(())
}

fn validate_f32_payload(bytes: &[u8], tensor: &TensorEntry) -> Result<(), String> {
    let start = as_usize(tensor.byte_offset)?;
    let len = as_usize(tensor.byte_length)?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "f32 tensor payload length overflow".to_string())?;
    let payload = bytes
        .get(start..end)
        .ok_or_else(|| "f32 tensor payload escapes the file".to_string())?;
    for chunk in payload.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if !value.is_finite() {
            return Err("f32 tensor payload contains non-finite values".to_string());
        }
    }
    Ok(())
}

fn validate_scales(bytes: &[u8], scale_offset: u64, count: usize) -> Result<(), String> {
    let start = as_usize(scale_offset)?;
    let len = count
        .checked_mul(4)
        .ok_or_else(|| "scale payload length overflow".to_string())?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "scale payload length overflow".to_string())?;
    let payload = bytes
        .get(start..end)
        .ok_or_else(|| "scale payload escapes the file".to_string())?;
    for chunk in payload.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if !value.is_finite() || value <= 0.0 {
            return Err("quantization scale payload contains non-positive values".to_string());
        }
    }
    Ok(())
}

fn expected_payload_len(tensor: &TensorEntry) -> Result<u64, String> {
    let elements = element_count(tensor);
    match tensor.dtype {
        DTYPE_F32 => elements
            .checked_mul(4)
            .ok_or_else(|| "f32 payload length overflow".to_string()),
        DTYPE_Q8_0 => Ok(elements),
        DTYPE_Q4_0 => {
            if !elements.is_multiple_of(2) {
                return Err("q4_0 tensor element count must be even".to_string());
            }
            Ok(elements / 2)
        }
        _ => Err("unsupported tensor dtype".to_string()),
    }
}

fn element_count(tensor: &TensorEntry) -> u64 {
    let mut total = 1u64;
    for dim in tensor.dims.iter().take(tensor.rank as usize) {
        total = total.saturating_mul(u64::from(*dim));
    }
    total
}

fn rows(tensor: &TensorEntry) -> Result<usize, String> {
    if tensor.rank <= 1 {
        Ok(1)
    } else {
        as_usize(u64::from(tensor.dims[0]))
    }
}

fn cols_per_row(tensor: &TensorEntry) -> Result<usize, String> {
    if tensor.rank <= 1 {
        return as_usize(element_count(tensor));
    }
    let mut cols = 1u64;
    for dim in tensor.dims.iter().skip(1).take(tensor.rank as usize - 1) {
        cols = cols
            .checked_mul(u64::from(*dim))
            .ok_or_else(|| "columns per row overflow".to_string())?;
    }
    as_usize(cols)
}

fn dims(tensor: &TensorEntry) -> Vec<u32> {
    tensor
        .dims
        .iter()
        .take(tensor.rank as usize)
        .copied()
        .collect()
}

fn quantization_label(tensors: &[TensorEntry]) -> &'static str {
    let mut has_f32 = false;
    let mut has_q8 = false;
    let mut has_q4 = false;
    for tensor in tensors {
        match tensor.dtype {
            DTYPE_F32 => has_f32 = true,
            DTYPE_Q8_0 => has_q8 = true,
            DTYPE_Q4_0 => has_q4 = true,
            _ => {}
        }
    }
    match (has_f32, has_q8, has_q4) {
        (true, false, false) => "f32",
        (false, true, false) => "q8_0",
        (false, false, true) => "q4_0",
        _ => "mixed",
    }
}

fn tokenizer_checksum(bytes: &[u8], header: &SlmHeader) -> Result<u64, String> {
    let start = as_usize(header.tokenizer_offset)?;
    let len = as_usize(header.tokenizer_length)?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "tokenizer checksum range overflow".to_string())?;
    let section = bytes
        .get(start..end)
        .ok_or_else(|| "tokenizer checksum range escapes the file".to_string())?;
    Ok(section_checksum(section, 0x746f6b656e697a65))
}

fn tensor_layout_checksum(tensors: &[TensorEntry]) -> u64 {
    let mut sorted = tensors.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|tensor| tensor.name_hash);
    let mut acc = 0x74656e736f722d6cu64;
    for tensor in sorted {
        mix_u64(&mut acc, tensor.name_hash);
        mix_u64(&mut acc, u64::from(tensor.dtype));
        mix_u64(&mut acc, u64::from(tensor.rank));
        for dim in tensor.dims {
            mix_u64(&mut acc, u64::from(dim));
        }
        mix_u64(&mut acc, u64::from(tensor.block_size));
        mix_u64(&mut acc, tensor.byte_length);
    }
    acc
}

fn section_checksum(bytes: &[u8], seed: u64) -> u64 {
    let mut acc = seed;
    for (index, byte) in bytes.iter().enumerate() {
        acc ^= u64::from(*byte).wrapping_add((index as u64) << 2);
        acc = acc.rotate_left(9).wrapping_mul(0x100000001b3);
    }
    acc
}

fn mix_u64(acc: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *acc ^= u64::from(byte);
        *acc = acc.rotate_left(11).wrapping_mul(0x100000001b3);
    }
}

fn shape_summary(header: &SlmHeader) -> String {
    format!(
        "vocab={},hidden={},layers={},heads={},kv_heads={},head_dim={},ffn={},context={}",
        header.vocab_size,
        header.hidden_size,
        header.layer_count,
        header.head_count,
        header.kv_head_count,
        header.head_dim,
        header.ffn_size,
        header.max_context
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| "u32 offset overflow".to_string())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "unexpected end while reading u32".to_string())?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "u64 offset overflow".to_string())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "unexpected end while reading u64".to_string())?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn range_within(file_len: usize, offset: u64, len: u64) -> bool {
    let Some(end) = offset.checked_add(len) else {
        return false;
    };
    end <= file_len as u64
}

fn simple_checksum(bytes: &[u8]) -> u64 {
    let mut acc = 0x9e3779b97f4a7c15u64;
    for (index, byte) in bytes.iter().enumerate() {
        let value = if (100..108).contains(&index) {
            0
        } else {
            *byte
        };
        acc ^= u64::from(value).wrapping_add((index as u64) << 1);
        acc = acc.rotate_left(7).wrapping_mul(0x100000001b3);
    }
    acc
}

fn name_hash(name: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn as_usize(value: u64) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| "value does not fit in usize".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn validates_sibling_q8_fixture_when_present() {
        let Some(path) = sibling_q8_fixture() else {
            eprintln!("skipping sibling q8 fixture test; TinyRustLM fixture not present");
            return;
        };
        let bytes = fs::read(path).unwrap();
        let report = validate_slm_bytes(&bytes).unwrap();
        assert_eq!(report.file_bytes, 8832);
        assert_eq!(
            report.model_shape,
            "vocab=260,hidden=8,layers=1,heads=2,kv_heads=2,head_dim=4,ffn=16,context=16"
        );
        assert_eq!(report.quantization, "q8_0");
        assert_eq!(report.checksum, "0x481f1037d57ebcd3");
        assert_eq!(report.tokenizer_checksum, "0x288f65d94c26743e");
        assert_eq!(report.tensor_layout_checksum, "0xd8fb6e636a8badd6");
    }

    fn sibling_q8_fixture() -> Option<std::path::PathBuf> {
        let rust_root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
        let path = rust_root
            .join("SLMComposer.com")
            .join("tinyrustlm")
            .join("models")
            .join("tiny-test-model-q8.slm");
        path.exists().then_some(path)
    }
}
