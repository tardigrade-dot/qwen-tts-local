use std::io::Write;

pub const SAMPLE_RATE: u32 = 24000;
pub const BYTES_PER_SAMPLE: u16 = 2;
pub const CHANNELS: u16 = 1;

pub fn write_wav_header<W: Write>(writer: &mut W, data_len: u32) -> std::io::Result<()> {
    let file_size = 36 + data_len;
    writer.write_all(b"RIFF")?;
    writer.write_all(&file_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;

    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?;
    writer.write_all(&CHANNELS.to_le_bytes())?;
    writer.write_all(&SAMPLE_RATE.to_le_bytes())?;

    let byte_rate = SAMPLE_RATE * CHANNELS as u32 * BYTES_PER_SAMPLE as u32;
    writer.write_all(&byte_rate.to_le_bytes())?;

    let block_align = CHANNELS * BYTES_PER_SAMPLE;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&(BYTES_PER_SAMPLE * 8).to_le_bytes())?;

    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;

    Ok(())
}

pub fn build_wav_in_memory(samples_f32: &[f32]) -> Vec<u8> {
    let s16_samples: Vec<i16> = samples_f32
        .iter()
        .map(|&s| {
            let clamped = if s > 1.0 {
                1.0
            } else if s < -1.0 {
                -1.0
            } else {
                s
            };
            (clamped * 32767.0) as i16
        })
        .collect();

    let data_len = (s16_samples.len() * 2) as u32;
    let mut buf = Vec::with_capacity(44 + data_len as usize);
    write_wav_header(&mut buf, data_len).unwrap();
    for sample in &s16_samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }
    buf
}

pub fn f32_to_s16le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let val = (clamped * 32767.0) as i16;
        out.extend_from_slice(&val.to_le_bytes());
    }
    out
}
