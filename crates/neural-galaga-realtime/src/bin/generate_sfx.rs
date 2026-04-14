use std::io::Write;

fn main() {
    let sample_rate = 22050u32;
    let duration = 0.2f32;
    let num_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = vec![0i16; num_samples];

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let pi2 = 2.0 * std::f32::consts::PI;

        // Bell/ding: fixed high pitch with inharmonic overtones
        let f1 = 1800.0; // fundamental
        let wave = (pi2 * f1 * t).sin() * 0.5
            + (pi2 * f1 * 2.76 * t).sin() * 0.25  // inharmonic — bell-like
            + (pi2 * f1 * 4.07 * t).sin() * 0.15   // another partial
            + (pi2 * f1 * 5.2 * t).sin() * 0.1; // shimmer

        // Sharp attack, long-ish ring decay
        let envelope = (-t * 18.0).exp();

        samples[i] = (wave * envelope * 11000.0) as i16;
    }

    // Write WAV file
    let out_dir = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));
    let path = format!("{out_dir}/shield_hit.wav");

    let data_size = (num_samples * 2) as u32;
    let file_size = 36 + data_size;

    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"RIFF").unwrap();
    f.write_all(&file_size.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();
    f.write_all(b"fmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    f.write_all(&1u16.to_le_bytes()).unwrap(); // mono
    f.write_all(&sample_rate.to_le_bytes()).unwrap();
    f.write_all(&(sample_rate * 2).to_le_bytes()).unwrap(); // byte rate
    f.write_all(&2u16.to_le_bytes()).unwrap(); // block align
    f.write_all(&16u16.to_le_bytes()).unwrap(); // bits per sample
    f.write_all(b"data").unwrap();
    f.write_all(&data_size.to_le_bytes()).unwrap();
    for s in &samples {
        f.write_all(&s.to_le_bytes()).unwrap();
    }

    println!("Generated {path} ({num_samples} samples, {duration}s)");

    // --- Powerup pickup: bright ascending two-note chime ---
    let pu_duration = 0.35f32;
    let pu_samples = (sample_rate as f32 * pu_duration) as usize;
    let mut pu_buf = vec![0i16; pu_samples];

    for i in 0..pu_samples {
        let t = i as f32 / sample_rate as f32;
        let pi2 = 2.0 * std::f32::consts::PI;

        // Two notes: C6 (1047Hz) for first half, E6 (1319Hz) for second half
        let freq = if t < 0.15 { 1047.0 } else { 1319.0 };
        let local_t = if t < 0.15 { t } else { t - 0.15 };

        let wave = (pi2 * freq * t).sin() * 0.45
            + (pi2 * freq * 2.0 * t).sin() * 0.25    // octave
            + (pi2 * freq * 3.0 * t).sin() * 0.15     // fifth above octave
            + (pi2 * freq * 5.0 * t).sin() * 0.05; // sparkle

        // Quick attack, moderate decay per note
        let envelope = (-local_t * 12.0).exp() * (1.0 - (-t * 80.0).exp());

        pu_buf[i] = (wave * envelope * 10000.0) as i16;
    }

    let pu_path = format!("{out_dir}/powerup_pickup.wav");
    let pu_data_size = (pu_samples * 2) as u32;
    let pu_file_size = 36 + pu_data_size;

    let mut f = std::fs::File::create(&pu_path).unwrap();
    f.write_all(b"RIFF").unwrap();
    f.write_all(&pu_file_size.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();
    f.write_all(b"fmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();
    f.write_all(&sample_rate.to_le_bytes()).unwrap();
    f.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
    f.write_all(&2u16.to_le_bytes()).unwrap();
    f.write_all(&16u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&pu_data_size.to_le_bytes()).unwrap();
    for s in &pu_buf {
        f.write_all(&s.to_le_bytes()).unwrap();
    }

    println!("Generated {pu_path} ({pu_samples} samples, {pu_duration}s)");
}
