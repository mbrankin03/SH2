use sh2_assets::*;
use std::fs;
use std::path::Path;

use crate::ROOT_ASSETS;

#[test]
fn test_sddata_audio_parsing() {
    let sddata_path = format!("{ROOT_ASSETS}/data/sound/sddata.bin");
    if !Path::new(&sddata_path).exists() {
        println!("Skipping test_sddata_audio_parsing: sddata.bin not found");
        return;
    }

    let data = fs::read(&sddata_path).expect("Failed to read sddata.bin");
    // Read first 1MB for fast unit test verification
    let sample_slice = &data[..1_000_000.min(data.len())];
    let bank = parse_sddata(sample_slice).expect("Failed to parse sound bank");
    assert!(
        !bank.entries.is_empty(),
        "Expected WAV entries in sddata slice"
    );
}

#[test]
fn test_mes_dialogue_parsing() {
    let mes_path = format!("{ROOT_ASSETS}/sh2e/etc/message/stage_toilet_msg_e.mes");
    if !Path::new(&mes_path).exists() {
        println!("Skipping test_mes_dialogue_parsing: mes file not found at {mes_path}");
        return;
    }

    let data = fs::read(&mes_path).expect("Failed to read stage_toilet_msg_e.mes");
    let table = parse_mes(&data).expect("Failed to parse .mes file");

    assert!(
        !table.messages.is_empty(),
        "Expected messages in .mes table"
    );
    let msg0 = table.get(0).expect("Expected message 0");
    assert!(
        msg0.text.contains("Mary..."),
        "Message 0 should contain 'Mary...', found: {}",
        msg0.text
    );
}

#[test]
fn test_adx_audio_decoding() {
    let adx_path = format!("{ROOT_ASSETS}/sh2e/sound/adx/apart/bgm_001.adx");
    if !Path::new(&adx_path).exists() {
        println!("Skipping test_adx_audio_decoding: adx file not found at {adx_path}");
        return;
    }

    let data = fs::read(&adx_path).expect("Failed to read bgm_001.adx");
    let header = parse_adx_header(&data).expect("Failed to parse ADX header");
    assert_eq!(header.channels, 2);
    assert_eq!(header.sample_rate, 48000);
    assert!(header.has_loop);
    assert!(header.loop_start_sample > 0);
    assert!(header.loop_end_sample > header.loop_start_sample);

    // Decode first slice (e.g. 50,000 bytes) to test decode pipeline speed
    let test_slice_len = 50_000.min(data.len());
    let mut truncated_data = data[..test_slice_len].to_vec();
    // adjust total samples in header to match available packets
    let avail_packets = (test_slice_len - header.data_offset) / (18 * header.channels as usize);
    let avail_samples = (avail_packets * 32) as u32;
    truncated_data[12..16].copy_from_slice(&avail_samples.to_be_bytes());

    let decoded = decode_adx(&truncated_data).expect("Failed to decode ADX slice");
    assert_eq!(decoded.frames.len(), avail_samples as usize);
}

#[test]
fn test_spu2_adpcm_shift_gt_12_maps_to_9() {
    // PS2 SPU2 hardware maps raw ADPCM shift values > 12 (13, 14, 15) to shift = 9.
    let make_block = |shift: u8| -> [u8; 16] {
        let mut blk = [0u8; 16];
        blk[0] = shift & 0x0F; // filter 0, raw shift
        blk[1] = 0x01; // end of sample
        for b in &mut blk[2..16] {
            *b = 0x73; // non-zero signed nibbles
        }
        blk
    };

    let ref_shift9 = decode_spu2_vag_to_wav(&make_block(9), 0, 22050).unwrap();
    let ref_shift12 = decode_spu2_vag_to_wav(&make_block(12), 0, 22050).unwrap();
    assert_ne!(ref_shift9.data, ref_shift12.data);

    for raw_shift in [13u8, 14, 15] {
        let decoded = decode_spu2_vag_to_wav(&make_block(raw_shift), 0, 22050).unwrap();
        assert_eq!(
            decoded.data, ref_shift9.data,
            "SPU2 ADPCM shift {raw_shift} must map to shift 9"
        );
    }
}

#[test]
fn test_aix_container_demux_and_decoding() {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let sound_dir = resolver.data_dir.join("sound/adx");
    if !sound_dir.exists() {
        println!("Skipping test_aix_container_demux_and_decoding: sound/adx not found");
        return;
    }

    // 1. Test bgm_118.aix (apart/bgm_118.aix) - multi-stream 5-channel ambient
    let bgm_118_path = sound_dir.join("apart/bgm_118.aix");
    if bgm_118_path.exists() {
        let data = fs::read(&bgm_118_path).expect("Failed to read bgm_118.aix");
        let raw_streams = formats::aix::demux_aix(&data).expect("Failed to demux bgm_118.aix");
        assert_eq!(raw_streams.len(), 5);

        // Test decoding full mixed ADX stereo
        let mixed =
            formats::aix::decode_aix_mixed(&data).expect("Failed to decode mixed bgm_118.aix");
        assert_eq!(mixed.header.sample_rate, 24000);
        assert_eq!(mixed.header.channels, 2);
        assert!(!mixed.frames.is_empty());
    }

    // 2. Test bgm_114_b.aix (forest/bgm_114_b.aix) - 6-stream 44.1kHz container
    let bgm_114_b_path = sound_dir.join("forest/bgm_114_b.aix");
    if bgm_114_b_path.exists() {
        let data = fs::read(&bgm_114_b_path).expect("Failed to read bgm_114_b.aix");
        let raw_streams = formats::aix::demux_aix(&data).expect("Failed to demux bgm_114_b.aix");
        assert_eq!(raw_streams.len(), 6);

        let mixed =
            formats::aix::decode_aix_mixed(&data).expect("Failed to decode mixed bgm_114_b.aix");
        assert_eq!(mixed.header.sample_rate, 44100);
        assert_eq!(mixed.header.channels, 2);
        assert!(!mixed.frames.is_empty());
    }
}

#[test]
fn test_sh2_61_dual_sddata_loop_flags_and_voice_afs_parsing() {
    // 1. Synthetic AFS container test (always runs in headless CI)
    let mut fake_wav = Vec::new();
    fake_wav.extend_from_slice(b"RIFF");
    fake_wav.extend_from_slice(&36u32.to_le_bytes());
    fake_wav.extend_from_slice(b"WAVEfmt ");
    fake_wav.extend_from_slice(&16u32.to_le_bytes());
    fake_wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    fake_wav.extend_from_slice(&1u16.to_le_bytes()); // 1 channel
    fake_wav.extend_from_slice(&48000u32.to_le_bytes()); // 48 kHz
    fake_wav.extend_from_slice(&96000u32.to_le_bytes());
    fake_wav.extend_from_slice(&2u16.to_le_bytes());
    fake_wav.extend_from_slice(&16u16.to_le_bytes());
    fake_wav.extend_from_slice(b"data");
    fake_wav.extend_from_slice(&4u32.to_le_bytes());
    fake_wav.extend_from_slice(&[0x10, 0x20, 0x30, 0x40]);

    let mut fake_afs = Vec::new();
    fake_afs.extend_from_slice(b"AFS\0");
    fake_afs.extend_from_slice(&1u32.to_le_bytes()); // 1 entry
    fake_afs.extend_from_slice(&16u32.to_le_bytes()); // offset 16
    fake_afs.extend_from_slice(&(fake_wav.len() as u32).to_le_bytes());
    fake_afs.extend_from_slice(&fake_wav);

    let afs_idx = parse_afs_index(&fake_afs).expect("Failed to parse synthetic AFS");
    assert_eq!(afs_idx.entries.len(), 1);
    let decoded_wav =
        decode_afs_entry_to_wav(&fake_afs, 0).expect("Failed to decode synthetic AFS entry");
    assert_eq!(decoded_wav.sample_rate, 48000);
    assert_eq!(decoded_wav.data.len(), 4);

    // 2. Verify synthetic SH2:EE WAV with loop flag `[0x00, 0x01]` before `LIST....INFO` chunk
    let mut ee_looped_wav = fake_wav.clone();
    ee_looped_wav.extend_from_slice(&[0x00, 0x01]); // Konami loop flag immediately after `data` chunk
    ee_looped_wav.extend_from_slice(b"LIST\x10\x00\x00\x00INFOICOP\x04\x00\x00\x00SH2\0");
    let ee_bank = parse_sddata(&ee_looped_wav).expect("Failed to parse EE looped WAV");
    assert_eq!(ee_bank.entries.len(), 1);
    assert!(
        ee_bank.entries[0].loops,
        "SH2:EE WAV with loop flag before LIST/INFO chunk must be marked as looping"
    );

    // 3. On-disk retail (`data/sound/sddata.bin`) & SH2:EE (`sh2e/sound/sddata.bin`) full 417-entry verification
    let data_sddata = format!("{ROOT_ASSETS}/data/sound/sddata.bin");
    let sh2e_sddata = format!("{ROOT_ASSETS}/sh2e/sound/sddata.bin");
    if Path::new(&data_sddata).exists() && Path::new(&sh2e_sddata).exists() {
        let orig_bytes = fs::read(&data_sddata).expect("read data/sound/sddata.bin");
        let orig_bank = parse_sddata(&orig_bytes).expect("parse data/sound/sddata.bin");
        assert_eq!(orig_bank.entries.len(), 417);

        let ee_bytes = fs::read(&sh2e_sddata).expect("read sh2e/sound/sddata.bin");
        let ee_bank = parse_sddata(&ee_bytes).expect("parse sh2e/sound/sddata.bin");
        assert_eq!(ee_bank.entries.len(), 417);
        assert_eq!(ee_bank.entries[0].sample_rate, 48000);

        // Both banks must mark entries 350..=366 and 416 as looping, and chainsaw (352) has loop_start > 0
        for idx in (350..=366).chain(std::iter::once(416)) {
            assert!(
                orig_bank.entries[idx].loops,
                "data/sound/sddata.bin[{idx}] must loop"
            );
            assert!(
                ee_bank.entries[idx].loops,
                "sh2e/sound/sddata.bin[{idx}] must loop"
            );
        }
        assert!(orig_bank.entries[352].loop_start > 0);
        assert!(ee_bank.entries[352].loop_start > 0);
    }

    // 4. Verify `voice.afs` index & entry decoding (e.g., Save Point chime `35` = `0x2743` and Game Start `24` = `0x3A9A`)
    let data_afs = format!("{ROOT_ASSETS}/data/sound/adx/voice/voice.afs");
    let sh2e_afs = format!("{ROOT_ASSETS}/sh2e/sound/adx/voice/voice.afs");
    if Path::new(&data_afs).exists() && Path::new(&sh2e_afs).exists() {
        let orig_afs = fs::read(&data_afs).expect("read data voice.afs");
        let orig_idx = parse_afs_index(&orig_afs).expect("parse data voice.afs");
        assert_eq!(orig_idx.entries.len(), 159);
        let orig_save = decode_afs_entry_to_wav(&orig_afs, 35).expect("decode data voice.afs[35]");
        assert_eq!(orig_save.sample_rate, 22050);
        assert!(!orig_save.data.is_empty());

        let ee_afs = fs::read(&sh2e_afs).expect("read sh2e voice.afs");
        let ee_idx = parse_afs_index(&ee_afs).expect("parse sh2e voice.afs");
        assert_eq!(ee_idx.entries.len(), 159);
        let ee_save = decode_afs_entry_to_wav(&ee_afs, 35).expect("decode sh2e voice.afs[35]");
        assert!(
            ee_save.sample_rate == 22050 || ee_save.sample_rate == 48000,
            "Expected 22050 Hz (ADX) or 48000 Hz (WAV) for sh2e voice.afs[35], got {}",
            ee_save.sample_rate
        );
        assert!(!ee_save.data.is_empty());

        // Also verify SH2:EE standalone override WAV `sh2e/sound/extra/save_sound.wav` parses cleanly
        let sh2e_extra_save = format!("{ROOT_ASSETS}/sh2e/sound/extra/save_sound.wav");
        if Path::new(&sh2e_extra_save).exists() {
            let extra_bytes = fs::read(&sh2e_extra_save).expect("read save_sound.wav");
            let extra_bank = parse_sddata(&extra_bytes).expect("parse save_sound.wav");
            assert_eq!(extra_bank.entries.len(), 1);
            assert!(extra_bank.entries[0].sample_rate >= 22050);
            assert!(!extra_bank.entries[0].data.is_empty());
        }
    }
}

#[test]
fn test_sh2_175_afs_seek_reader_single_entry_and_error_bounds() {
    use std::io::Cursor;

    let make_pcm_wav = |sample_rate: u32, payload: &[u8]| -> Vec<u8> {
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + payload.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        wav.extend_from_slice(payload);
        wav
    };

    let wav0 = make_pcm_wav(22050, &[0x01, 0x02, 0x03, 0x04]);
    let wav1 = make_pcm_wav(48000, &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

    let off0 = 8u32 + 2 * 8; // header (8) + 2 TOC entries (16) = 24
    let off1 = off0 + wav0.len() as u32;

    let mut afs = Vec::new();
    afs.extend_from_slice(b"AFS\0");
    afs.extend_from_slice(&2u32.to_le_bytes());
    afs.extend_from_slice(&off0.to_le_bytes());
    afs.extend_from_slice(&(wav0.len() as u32).to_le_bytes());
    afs.extend_from_slice(&off1.to_le_bytes());
    afs.extend_from_slice(&(wav1.len() as u32).to_le_bytes());
    afs.extend_from_slice(&wav0);
    afs.extend_from_slice(&wav1);

    // Reading entry 1 via seek-reader matches slice decoder identically
    let mut cursor = Cursor::new(afs.as_slice());
    let raw1 = read_afs_entry_from_reader(&mut cursor, 1).expect("read entry 1");
    assert_eq!(raw1, wav1);

    let decoded1 = decode_afs_entry_from_reader(&mut cursor, 1).expect("decode entry 1");
    let full1 = decode_afs_entry_to_wav(&afs, 1).expect("full decode entry 1");
    assert_eq!(decoded1.sample_rate, 48000);
    assert_eq!(decoded1.data, full1.data);

    let decoded0 = decode_afs_entry_from_reader(&mut cursor, 0).expect("decode entry 0");
    assert_eq!(decoded0.sample_rate, 22050);
    assert_eq!(decoded0.data, vec![0x01, 0x02, 0x03, 0x04]);

    // Out-of-bounds index -> InvalidData
    assert!(matches!(
        read_afs_entry_from_reader(&mut cursor, 2),
        Err(AssetError::InvalidData(_))
    ));

    // Invalid magic -> InvalidMagic
    let mut bad_magic = Cursor::new(b"NOPE\x00\x00\x00\x00".as_slice());
    assert!(matches!(
        read_afs_entry_from_reader(&mut bad_magic, 0),
        Err(AssetError::InvalidMagic { .. })
    ));

    // Truncated header -> BufferUnderflow
    let mut short_hdr = Cursor::new(b"AFS\0\x01".as_slice());
    assert!(matches!(
        read_afs_entry_from_reader(&mut short_hdr, 0),
        Err(AssetError::BufferUnderflow { .. })
    ));

    // Truncated TOC (claims 10 entries, only 8 bytes present) -> BufferUnderflow
    let mut short_toc = Cursor::new(b"AFS\0\x0A\x00\x00\x00".as_slice());
    assert!(matches!(
        read_afs_entry_from_reader(&mut short_toc, 0),
        Err(AssetError::BufferUnderflow { .. })
    ));

    // Entry byte range past EOF -> BufferUnderflow
    let mut bad_range = Vec::new();
    bad_range.extend_from_slice(b"AFS\0");
    bad_range.extend_from_slice(&1u32.to_le_bytes());
    bad_range.extend_from_slice(&16u32.to_le_bytes());
    bad_range.extend_from_slice(&9999u32.to_le_bytes());
    let mut bad_range_cursor = Cursor::new(bad_range.as_slice());
    assert!(matches!(
        read_afs_entry_from_reader(&mut bad_range_cursor, 0),
        Err(AssetError::BufferUnderflow { .. })
    ));

    // Retail on-disk `voice.afs` seek-read parity check (when retail assets are present)
    let data_afs = format!("{ROOT_ASSETS}/data/sound/adx/voice/voice.afs");
    if Path::new(&data_afs).exists() {
        let mut file = fs::File::open(&data_afs).expect("open data voice.afs");
        let seek_wav =
            decode_afs_entry_from_reader(&mut file, 35).expect("seek-decode data voice.afs[35]");
        assert_eq!(seek_wav.sample_rate, 22050);
        assert!(!seek_wav.data.is_empty());
    }
}

#[test]
fn test_sh2_62_ps2_prototype_sdb_sfc_vag_and_wav_slicing() {
    // 1. Test `parse_ps2_sdb_sound_data` on synthetic 64-byte `_SOUND_DATA` records + `-0x8000_0000` terminator
    let mut sdb_bytes = Vec::new();
    // Record 0 (sentinel 9900000256.0_f32 = 0x50138581)
    for _ in 0..8 {
        sdb_bytes.extend_from_slice(&9900000256.0f32.to_le_bytes());
    }
    sdb_bytes.extend_from_slice(&0i32.to_le_bytes()); // flags = 0
    for ch in 0..7u32 {
        sdb_bytes.extend_from_slice(&(0x21 + ch).to_le_bytes());
    }
    // Record 1 (valid room bounding box)
    let coords = [
        -25400.0f32,
        -17550.0,
        -25400.0,
        -22950.0,
        -9050.0,
        -22950.0,
        0.0,
        -3000.0,
    ];
    for c in coords {
        sdb_bytes.extend_from_slice(&c.to_le_bytes());
    }
    sdb_bytes.extend_from_slice(&0i32.to_le_bytes()); // flags = 0
    for _ in 0..7 {
        sdb_bytes.extend_from_slice(&0x15u32.to_le_bytes());
    }
    // Record 2 (terminator record with flags = -2147483648 / 0x80000000)
    for _ in 0..8 {
        sdb_bytes.extend_from_slice(&0.0f32.to_le_bytes());
    }
    sdb_bytes.extend_from_slice(&0x8000_0000u32.to_le_bytes());
    for _ in 0..7 {
        sdb_bytes.extend_from_slice(&0u32.to_le_bytes());
    }

    let parsed_sdb =
        parse_ps2_sdb_sound_data(&sdb_bytes).expect("Failed to parse PS2 .sdb records");
    assert_eq!(parsed_sdb.len(), 2);
    assert_eq!(parsed_sdb[0].x0, 9900000256.0);
    assert_eq!(parsed_sdb[0].chanstat[0], 0x21);
    assert_eq!(parsed_sdb[1].x0, -25400.0);
    assert_eq!(parsed_sdb[1].x2, -9050.0);
    assert_eq!(parsed_sdb[1].min_y, -3000.0);
    assert_eq!(parsed_sdb[1].chanstat[0], 0x15);

    // 2. Test `parse_ps2_sfc_pcm` for PS2 S-Force 3D positional audio `.sfc` banks
    let raw_sfc = vec![0x34u8, 0x12, 0xCD, 0xAB, 0x00, 0x01, 0xFF];
    let sfc_wav = parse_ps2_sfc_pcm(&raw_sfc, 15, 22050).expect("Failed to parse .sfc PCM");
    assert_eq!(sfc_wav.id, 15);
    assert_eq!(sfc_wav.channels, 1);
    assert_eq!(sfc_wav.sample_rate, 22050);
    assert_eq!(sfc_wav.data.len(), 6); // 2-byte aligned

    // 3. Test `decode_spu2_vag_to_wav` and `parse_ps2_sound_dat_bank` (`IECSsreV` / `IECSdaeH` / `IECSigaV`)
    let mut bank_buf = vec![0u8; 0x80];
    bank_buf[0..8].copy_from_slice(&PS2_VAG_VERS_MAGIC);
    bank_buf[0x10..0x18].copy_from_slice(&PS2_VAG_HEAD_MAGIC);
    bank_buf[0x18..0x1C].copy_from_slice(&0x60u32.to_le_bytes()); // hd_size = 0x60
    bank_buf[0x1C..0x20].copy_from_slice(&0x20u32.to_le_bytes()); // bd_size = 0x20 (2 SPU2 blocks)
    bank_buf[0x24..0x28].copy_from_slice(&0x38u32.to_le_bytes()); // vagi_offset = 0x38

    // IECSigaV at 0x38: 1 sample (max_idx = 0), descriptor at +0x18 (`0x50`)
    bank_buf[0x38..0x40].copy_from_slice(&PS2_VAG_VAGI_MAGIC);
    bank_buf[0x40..0x44].copy_from_slice(&0x20u32.to_le_bytes()); // chunk_size
    bank_buf[0x44..0x48].copy_from_slice(&0u32.to_le_bytes()); // max_idx = 0 (count = 1)
    bank_buf[0x48..0x4C].copy_from_slice(&0x18u32.to_le_bytes()); // rel_ofs to descriptor -> 0x38 + 0x18 = 0x50
    bank_buf[0x50..0x54].copy_from_slice(&0u32.to_le_bytes()); // bd_offset = 0
    bank_buf[0x54..0x56].copy_from_slice(&22050u16.to_le_bytes()); // sample_rate = 22050 Hz

    // BD at 0x60..0x80: Block 0 = loop start (`flags = 0x04`), Block 1 = loop end + repeat (`flags = 0x03`)
    bank_buf[0x60] = 0x12; // filter 1, shift 2
    bank_buf[0x61] = 0x04; // loop start
    for b in &mut bank_buf[0x62..0x70] {
        *b = 0x21;
    }
    bank_buf[0x70] = 0x12;
    bank_buf[0x71] = 0x03; // loop end + sustain
    for b in &mut bank_buf[0x72..0x80] {
        *b = 0x12;
    }

    let vag_bank =
        parse_ps2_sound_dat_bank(&bank_buf).expect("Failed to parse PS2 SOUND.DAT VAG bank");
    assert_eq!(vag_bank.entries.len(), 1);
    let vag_wav = &vag_bank.entries[0];
    assert_eq!(vag_wav.sample_rate, 22050);
    assert_eq!(vag_wav.channels, 1);
    assert_eq!(vag_wav.data.len(), 2 * 28 * 2); // 2 blocks * 28 samples * 2 bytes
    assert!(vag_wav.loops);
    assert_eq!(vag_wav.loop_start, 0);
    assert_eq!(vag_wav.loop_end, 56);

    // 4. Test `slice_wav_time_range` for slicing `SCENE_45` voiceovers
    let mut full_pcm = Vec::with_capacity(1000 * 2);
    for i in 0..1000i16 {
        full_pcm.extend_from_slice(&(i * 10).to_le_bytes());
    }
    let test_wav = WavEntry {
        id: 133,
        channels: 1,
        sample_rate: 100, // 100 Hz -> 10.0 seconds total
        bits_per_sample: 16,
        data: full_pcm,
        loops: false,
        loop_start: 0,
        loop_end: 0,
    };
    let sliced = slice_wav_time_range(&test_wav, 2.5, 5.5); // 3.0s = 300 samples = 600 bytes
    assert_eq!(sliced.data.len(), 600);
    assert_eq!(sliced.sample_rate, 100);
}
