use open_360_link_protocol::*;

/// Builds a synthetic USB Configuration Descriptor containing:
/// - Standard Interface Descriptor for UVC VideoControl (`bInterfaceNumber = 2`, `CC_VIDEO = 0x0e`, `SC_VIDEOCONTROL = 0x01`)
/// - `VC_INPUT_TERMINAL` (`bDescriptorSubtype = 0x02`, `bTerminalID = 3`, `wTerminalType = 0x0201` `ITT_CAMERA`)
/// - `VC_PROCESSING_UNIT` (`bDescriptorSubtype = 0x05`, `bUnitID = 5`)
/// - `VC_EXTENSION_UNIT` (`bDescriptorSubtype = 0x06`, `bUnitID = 11`) with `XU1_GUID_BYTES`
/// - `VC_EXTENSION_UNIT` (`bDescriptorSubtype = 0x06`, `bUnitID = 12`) with `XU2_GUID_BYTES`
/// - Standard Interface Descriptor for UVC VideoStreaming (`bInterfaceNumber = 3`, `CC_VIDEO = 0x0e`, `SC_VIDEOSTREAMING = 0x02`)
///   followed by a dummy `CS_INTERFACE` descriptor that should be ignored by `parse_uvc_vc_descriptors`.
#[test]
fn test_parse_uvc_vc_descriptors_synthetic() {
    let mut desc = Vec::new();

    // 1. Standard Interface Descriptor (USB_DT_INTERFACE = 0x04): bInterfaceNumber = 2, CC_VIDEO (0x0e), SC_VIDEOCONTROL (0x01)
    desc.extend_from_slice(&[
        9,    // bLength
        0x04, // bDescriptorType = USB_DT_INTERFACE
        2,    // bInterfaceNumber = 2
        0,    // bAlternateSetting
        1,    // bNumEndpoints
        0x0e, // bInterfaceClass = CC_VIDEO
        0x01, // bInterfaceSubClass = SC_VIDEOCONTROL
        0x00, // bInterfaceProtocol
        0,    // iInterface
    ]);

    // 2. VC_INPUT_TERMINAL (CS_INTERFACE = 0x24, subtype = 0x02, bTerminalID = 3, wTerminalType = 0x0201 ITT_CAMERA)
    desc.extend_from_slice(&[
        18,   // bLength
        0x24, // bDescriptorType = CS_INTERFACE
        0x02, // bDescriptorSubtype = VC_INPUT_TERMINAL
        3,    // bTerminalID = 3
        0x01, 0x02, // wTerminalType = 0x0201 (ITT_CAMERA)
        0,    // bAssocTerminal
        0,    // iTerminal
        0, 0, 0, 0, 0, 0, 3, 0xff, 0xff, 0x0f,
    ]);

    // 3. VC_PROCESSING_UNIT (CS_INTERFACE = 0x24, subtype = 0x05, bUnitID = 5)
    desc.extend_from_slice(&[
        13,   // bLength
        0x24, // bDescriptorType = CS_INTERFACE
        0x05, // bDescriptorSubtype = VC_PROCESSING_UNIT
        5,    // bUnitID = 5
        3,    // bSourceID = 3
        0, 0, // wMaxMultiplier
        3,    // bControlSize
        0xff, 0xff, 0x00, // bmControls
        0,    // iProcessing
        0,    // bmVideoStandards
    ]);

    // 4. VC_EXTENSION_UNIT for XU1 (CS_INTERFACE = 0x24, subtype = 0x06, bUnitID = 11, guid = XU1_GUID_BYTES)
    let mut xu1_desc = vec![
        24,   // bLength
        0x24, // bDescriptorType = CS_INTERFACE
        0x06, // bDescriptorSubtype = VC_EXTENSION_UNIT
        11,   // bUnitID = 11
    ];
    xu1_desc.extend_from_slice(&XU1_GUID_BYTES);
    xu1_desc.extend_from_slice(&[15, 1, 5, 0]);
    assert_eq!(xu1_desc.len(), 24);
    desc.extend_from_slice(&xu1_desc);

    // 5. VC_EXTENSION_UNIT for XU2 (CS_INTERFACE = 0x24, subtype = 0x06, bUnitID = 12, guid = XU2_GUID_BYTES)
    let mut xu2_desc = vec![
        24,   // bLength
        0x24, // bDescriptorType = CS_INTERFACE
        0x06, // bDescriptorSubtype = VC_EXTENSION_UNIT
        12,   // bUnitID = 12
    ];
    xu2_desc.extend_from_slice(&XU2_GUID_BYTES);
    xu2_desc.extend_from_slice(&[15, 1, 11, 0]);
    assert_eq!(xu2_desc.len(), 24);
    desc.extend_from_slice(&xu2_desc);

    // 6. Standard Interface Descriptor for VideoStreaming (SC_VIDEOSTREAMING = 0x02)
    // Descriptors inside VideoStreaming must NOT overwrite VC unit IDs.
    desc.extend_from_slice(&[
        9,    // bLength
        0x04, // bDescriptorType = USB_DT_INTERFACE
        3,    // bInterfaceNumber = 3
        0,    // bAlternateSetting
        0,    // bNumEndpoints
        0x0e, // bInterfaceClass = CC_VIDEO
        0x02, // bInterfaceSubClass = SC_VIDEOSTREAMING
        0x00, // bInterfaceProtocol
        0,    // iInterface
    ]);
    desc.extend_from_slice(&[
        6,    // bLength
        0x24, // CS_INTERFACE inside VideoStreaming (should be ignored)
        0x05, // subtype 0x05
        99,   // would be unit 99 if not ignored
        0, 0,
    ]);

    let parsed = parse_uvc_vc_descriptors(&desc);
    assert_eq!(parsed.control_interface, 2);
    assert_eq!(parsed.input_terminal_id, 3);
    assert_eq!(parsed.processing_unit_id, 5);
    assert_eq!(parsed.extension_unit_id, 11);
    assert_eq!(parsed.extension_unit2_id, 12);
}

#[test]
fn test_camera_models_pids_capabilities_and_video_mode_len() {
    // Verify Vendor ID and device filter
    assert_eq!(INSTA360_VENDOR_ID, 0x2e1a);
    for &pid in &[
        pids::LINK_1,
        pids::PUC3_GIMBAL_A,
        pids::LINK_2C,
        pids::LINK_2,
        pids::PUC3_HOST,
        pids::PUC3_GIMBAL_B,
        pids::LINK_SIMPLE_CLASS,
    ] {
        assert!(is_insta360_link_device(INSTA360_VENDOR_ID, pid));
        assert!(!is_insta360_link_device(0x1234, pid));
    }
    assert!(!is_insta360_link_device(INSTA360_VENDOR_ID, 0x9999));

    // Verify PID -> CameraModel mappings, internal codes, firmware filenames, and SetVideoMode payload lengths
    let link1 = CameraModel::from_pid(pids::LINK_1);
    assert_eq!(link1, CameraModel::Link);
    assert_eq!(link1.pid(), 0x4c01);
    assert_eq!(link1.internal_code(), "link1");
    assert_eq!(link1.firmware_filename(), Some(r"a:\Insta360WebCamFW.bin"));
    assert_eq!(link1.video_mode_payload_len(), 8);

    let puc3_a = CameraModel::from_pid(pids::PUC3_GIMBAL_A);
    assert_eq!(puc3_a, CameraModel::Puc3Gimbal(0x4c02));
    assert_eq!(puc3_a.pid(), 0x4c02);
    assert_eq!(puc3_a.internal_code(), "link2_gimbal");
    assert_eq!(puc3_a.firmware_filename(), Some(r"a:\Insta360PUC3FW_GIMBAL.bin"));
    assert_eq!(puc3_a.video_mode_payload_len(), 8);

    let link2c = CameraModel::from_pid(pids::LINK_2C);
    assert_eq!(link2c, CameraModel::Link2C);
    assert_eq!(link2c.pid(), 0x4c03);
    assert_eq!(link2c.internal_code(), "link2_main");
    assert_eq!(link2c.firmware_filename(), Some(r"a:\Insta360LINK2CFW_HOST.bin"));
    assert_eq!(link2c.video_mode_payload_len(), 36);

    let link2 = CameraModel::from_pid(pids::LINK_2);
    assert_eq!(link2, CameraModel::Link2);
    assert_eq!(link2.pid(), 0x4c04);
    assert_eq!(link2.internal_code(), "link2_gimbal");
    assert_eq!(link2.firmware_filename(), Some(r"a:\Insta360LINK2FW_GIMBAL.bin"));
    assert_eq!(link2.video_mode_payload_len(), 8);

    let puc3_host = CameraModel::from_pid(pids::PUC3_HOST);
    assert_eq!(puc3_host, CameraModel::Puc3Host);
    assert_eq!(puc3_host.pid(), 0x4c05);
    assert_eq!(puc3_host.internal_code(), "link2_main");
    assert_eq!(puc3_host.firmware_filename(), Some(r"a:\Insta360PUC3FW_HOST.bin"));
    assert_eq!(puc3_host.video_mode_payload_len(), 36);

    let puc3_b = CameraModel::from_pid(pids::PUC3_GIMBAL_B);
    assert_eq!(puc3_b, CameraModel::Puc3Gimbal(0x4c06));
    assert_eq!(puc3_b.pid(), 0x4c06);
    assert_eq!(puc3_b.internal_code(), "link2_gimbal");
    assert_eq!(puc3_b.firmware_filename(), Some(r"a:\Insta360PUC3FW_GIMBAL.bin"));
    assert_eq!(puc3_b.video_mode_payload_len(), 8);

    let dfu = CameraModel::from_pid(pids::LINK_SIMPLE_CLASS);
    assert_eq!(dfu, CameraModel::LinkSimpleClass);
    assert_eq!(dfu.pid(), 0x00c2);
    assert_eq!(dfu.internal_code(), "link1");
    assert_eq!(dfu.firmware_filename(), Some(r"a:\Insta360WebCamFW.bin"));
    assert_eq!(dfu.video_mode_payload_len(), 36);

    // Capability matrix assertions (`Webcam::CameraDeviceSupporter::UpdateUIStatus` at `0x10004a8b8`)
    let cap_link1 = DeviceCapabilities::for_pid(pids::LINK_1);
    assert!(cap_link1.has_physical_gimbal);
    assert!(cap_link1.support_roll_adjustment);
    assert!(!cap_link1.support_auto_framing);
    assert!(!cap_link1.support_privacy_mode);
    assert!(!cap_link1.support_audio_capture_mode);
    assert!(!cap_link1.support_horizontal_correction);
    assert!(!cap_link1.support_sound_wall);

    let cap_link2 = DeviceCapabilities::for_pid(pids::LINK_2);
    assert!(cap_link2.has_physical_gimbal);
    assert!(!cap_link2.support_roll_adjustment);
    assert!(cap_link2.support_auto_framing);
    assert!(cap_link2.support_privacy_mode);
    assert!(cap_link2.support_area_track);
    assert!(cap_link2.support_speak_tracking);
    assert!(cap_link2.support_audio_capture_mode);
    assert!(cap_link2.support_horizontal_correction);
    assert!(!cap_link2.support_sound_wall);

    let cap_link2c = DeviceCapabilities::for_pid(pids::LINK_2C);
    assert!(!cap_link2c.has_physical_gimbal);
    assert!(!cap_link2c.support_ptz_drag);
    assert!(!cap_link2c.support_track_speed);
    assert!(cap_link2c.support_auto_framing);
    assert!(cap_link2c.support_image_template);
    assert!(cap_link2c.support_privacy_mode);
    assert!(cap_link2c.support_audio_capture_mode);
    assert!(!cap_link2c.support_area_track);
    assert!(!cap_link2c.support_horizontal_correction);

    let cap_puc3_b = DeviceCapabilities::for_pid(pids::PUC3_GIMBAL_B);
    assert!(cap_puc3_b.has_physical_gimbal);
    assert!(cap_puc3_b.support_area_track);
    assert!(cap_puc3_b.support_speak_tracking);
    assert!(cap_puc3_b.support_sound_wall);
    assert!(!cap_puc3_b.support_horizontal_correction);

    let cap_puc3_host = DeviceCapabilities::for_pid(pids::PUC3_HOST);
    assert!(!cap_puc3_host.has_physical_gimbal);
    assert!(cap_puc3_host.support_sound_wall);
    assert!(cap_puc3_host.support_auto_framing);
}

#[test]
fn test_controller_ptz_presets_and_gimbal_commands() {
    let mock = MockCameraTransport::new(CameraModel::Link2);
    let ctrl = Insta360CameraController::new(mock.clone());

    // 1. Absolute Pan/Tilt & Zoom
    ctrl.set_pantilt_absolute(36000, -18000).unwrap();
    assert_eq!(ctrl.get_pantilt_absolute().unwrap(), (36000, -18000));

    ctrl.set_zoom(250).unwrap();
    assert_eq!(ctrl.get_zoom().unwrap(), 250);
    let zoom_range = ctrl.get_zoom_range().unwrap();
    assert_eq!((zoom_range.min, zoom_range.max, zoom_range.default), (100, 400, 100));

    // 2. Relative Pan/Tilt (XU1 0x0b: [pan_speed, pan_dir, tilt_speed, tilt_dir])
    ctrl.set_pantilt_relative_ext(5, -1, 3, 1).unwrap();
    ctrl.stop_pantilt_relative().unwrap();

    // 3. Presets (XU1 0x01: [sub_op, index, 0, 0])
    ctrl.add_preset_position(4).unwrap();
    let flags = ctrl.get_preset_flags().unwrap();
    assert_ne!(flags.slot_mask & (1 << 4), 0);
    assert_eq!(flags.current_slot, 4);

    ctrl.recall_preset_position(1).unwrap();
    assert_eq!(ctrl.get_preset_flags().unwrap().current_slot, 1);

    ctrl.delete_preset_position(4).unwrap();
    assert_eq!(ctrl.get_preset_flags().unwrap().slot_mask & (1 << 4), 0);

    // 4. Composition bias & Reset Gimbal
    ctrl.set_composition_bias(-120, 80).unwrap();
    ctrl.reset_composition_bias().unwrap();
    ctrl.reset_gimbal().unwrap();
    assert_eq!(ctrl.get_pantilt_absolute().unwrap(), (0, 0));
    assert_eq!(ctrl.get_zoom().unwrap(), 100);

    // 5. Host Digital PTZ (XU2 0x09)
    let host_crop = HostPtzInfo {
        x: 500,
        y: 600,
        width: 2570,
        height: 2570,
    };
    ctrl.set_host_ptz(&host_crop).unwrap();

    // Inspect command log for exact unit IDs, selectors, and payload bytes
    let log = mock.command_log();
    assert_eq!(
        log[0],
        (
            UvcRequest::SetCur,
            DEFAULT_INPUT_TERMINAL_ID,
            ct_selectors::PANTILT_ABSOLUTE,
            [36000i32.to_le_bytes(), (-18000i32).to_le_bytes()].concat()
        )
    );
    assert_eq!(
        log[1],
        (
            UvcRequest::SetCur,
            DEFAULT_INPUT_TERMINAL_ID,
            ct_selectors::ZOOM_ABSOLUTE,
            250u16.to_le_bytes().to_vec()
        )
    );
    assert_eq!(
        log[2],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::PANTILT_RELATIVE_EXT,
            vec![0xff, 5, 1, 3]
        )
    );
    assert_eq!(
        log[3],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::PANTILT_RELATIVE_EXT,
            vec![0, 1, 0, 1]
        )
    );
    assert_eq!(
        log[4],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            vec![0x01, 4, 0x00, 0x00]
        )
    );
    assert_eq!(
        log[5],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            vec![0x00, 1, 0x00, 0x00]
        )
    );
    assert_eq!(
        log[6],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            vec![0x02, 4, 0x00, 0x00]
        )
    );
    assert_eq!(
        log[7],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            vec![0x06, (-120i16).to_le_bytes()[0], (-120i16).to_le_bytes()[1], 80, 0]
        )
    );
    assert_eq!(
        log[8],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            vec![0x06, 0xff, 0x7f, 0xff, 0x7f]
        )
    );
    assert_eq!(
        log[9],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::SYSTEM_AND_PRESET_CMD,
            vec![0x03, 0x00, 0x00, 0x00]
        )
    );
    assert_eq!(
        log[10],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT2_ID,
            xu2_selectors::HOST_PTZ,
            [
                500u16.to_le_bytes(),
                600u16.to_le_bytes(),
                2570u16.to_le_bytes(),
                2570u16.to_le_bytes()
            ]
            .concat()
        )
    );
}

#[test]
fn test_video_mode_8byte_vs_36byte_and_tracking_gestures() {
    // 1. Link 2 uses 8-byte SetVideoMode payload
    let mock_link2 = MockCameraTransport::new(CameraModel::Link2);
    let ctrl_link2 = Insta360CameraController::new(mock_link2.clone());
    let aux = VideoModeAuxData {
        flags: 1,
        rect: [10, 20, 300, 400],
        pitch: -450,
        zoom: 150,
        extra: [0xaa; 14],
    };
    ctrl_link2.set_video_mode(VideoMode::DeskView, &aux).unwrap();
    let log2 = mock_link2.command_log();
    assert_eq!(log2.len(), 1);
    assert_eq!(log2[0].1, DEFAULT_EXTENSION_UNIT_ID);
    assert_eq!(log2[0].2, xu1_selectors::VIDEO_MODE);
    assert_eq!(log2[0].3.len(), 8);
    assert_eq!(log2[0].3[0], VideoMode::DeskView as u8);
    assert_eq!(log2[0].3[1], 1);

    // 2. Link 2C uses 36-byte SetVideoMode payload
    let mock_link2c = MockCameraTransport::new(CameraModel::Link2C);
    let ctrl_link2c = Insta360CameraController::new(mock_link2c.clone());
    ctrl_link2c.set_video_mode(VideoMode::AutoFraming, &aux).unwrap();
    let log2c = mock_link2c.command_log();
    assert_eq!(log2c.len(), 1);
    assert_eq!(log2c[0].1, DEFAULT_EXTENSION_UNIT_ID);
    assert_eq!(log2c[0].2, xu1_selectors::VIDEO_MODE);
    assert_eq!(log2c[0].3.len(), 36);
    let (decoded_mode, decoded_aux) = ctrl_link2c.get_video_mode().unwrap();
    assert_eq!(decoded_mode, VideoMode::AutoFraming);
    assert_eq!(decoded_aux.rect, [10, 20, 300, 400]);
    assert_eq!(decoded_aux.pitch, -450);
    assert_eq!(decoded_aux.zoom, 150);

    // 3. Tracking speed, composition style, and gestures on XU1 0x03
    ctrl_link2.set_track_speed(TrackSpeed::Fast).unwrap();
    assert_eq!(ctrl_link2.get_track_speed().unwrap(), TrackSpeed::Fast);

    ctrl_link2.set_composition_style(CompositionStyle::WholeBody).unwrap();
    assert_eq!(ctrl_link2.get_composition_style().unwrap(), CompositionStyle::WholeBody);

    ctrl_link2.set_gesture_enabled(Gesture::VShape, false).unwrap();
    let gestures = ctrl_link2.get_gesture_status().unwrap();
    assert_eq!(gestures[3], (Gesture::VShape, false));
    assert_eq!(gestures[0], (Gesture::Ok, true));

    ctrl_link2.set_all_gestures_enabled(false).unwrap();

    let log2_after = mock_link2.command_log();
    // TrackSpeed::Fast -> [5, 2, 0..0] (16 bytes)
    let mut expected_speed = vec![0u8; 16];
    expected_speed[0] = 0x05;
    expected_speed[1] = TrackSpeed::Fast as u8;
    assert_eq!(
        log2_after[1],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::TRACKING_AND_GESTURE,
            expected_speed
        )
    );

    // CompositionStyle::WholeBody -> [6, 2, 0..0] (16 bytes)
    let mut expected_style = vec![0u8; 16];
    expected_style[0] = 0x06;
    expected_style[1] = CompositionStyle::WholeBody as u8;
    assert_eq!(
        log2_after[2],
        (
            UvcRequest::SetCur,
            DEFAULT_EXTENSION_UNIT_ID,
            xu1_selectors::TRACKING_AND_GESTURE,
            expected_style
        )
    );
}

#[test]
fn test_image_controls_exposure_wb_audio_filters_templates_and_extend_functions() {
    let mock = MockCameraTransport::new(CameraModel::Link2);
    let ctrl = Insta360CameraController::new(mock.clone());

    // 1. Standard Processing Unit (PU) controls
    ctrl.set_brightness(72).unwrap();
    assert_eq!(ctrl.get_brightness().unwrap(), 72);

    ctrl.set_contrast(64).unwrap();
    assert_eq!(ctrl.get_contrast().unwrap(), 64);

    ctrl.set_saturation(80).unwrap();
    assert_eq!(ctrl.get_saturation().unwrap(), 80);

    ctrl.set_sharpness(45).unwrap();
    assert_eq!(ctrl.get_sharpness().unwrap(), 45);

    ctrl.set_auto_white_balance(false).unwrap();
    assert!(!ctrl.get_auto_white_balance().unwrap());

    ctrl.set_white_balance_temperature(6500).unwrap();
    assert_eq!(ctrl.get_white_balance_temperature().unwrap(), 6500);

    ctrl.set_power_line_frequency(PowerLineFrequency::Hz50).unwrap();
    assert_eq!(ctrl.get_power_line_frequency().unwrap(), PowerLineFrequency::Hz50);

    // 2. Standard Camera Terminal (CT) focus, roll, shutter
    ctrl.set_auto_focus(false).unwrap();
    assert!(!ctrl.get_auto_focus().unwrap());
    ctrl.set_focus(85).unwrap();
    assert_eq!(ctrl.get_focus().unwrap(), 85);
    ctrl.set_roll(-150).unwrap();
    assert_eq!(ctrl.get_roll().unwrap(), -150);
    ctrl.set_shutter_speed(500).unwrap();
    assert_eq!(ctrl.get_shutter_speed().unwrap(), 500);

    // 3. Exposure Curve / Auto Exposure / ISO (XU1 0x07, 259 bytes) & EV Compensation (XU1 0x08, 4 bytes)
    ctrl.set_auto_exposure(false).unwrap();
    assert!(!ctrl.get_auto_exposure().unwrap());
    ctrl.set_iso(800).unwrap();
    assert_eq!(ctrl.get_iso().unwrap(), 800);
    ctrl.set_exposure_compensation(-1.5).unwrap();
    assert!((ctrl.get_exposure_compensation().unwrap() - (-1.5)).abs() < 1e-6);

    // 4. Audio Control (XU1 0x04, 2 bytes)
    ctrl.set_audio_noise_reduction(false).unwrap();
    assert!(!ctrl.get_audio_noise_reduction().unwrap());
    ctrl.set_audio_capture_mode(AudioCaptureMode::MusicBalance).unwrap();
    assert_eq!(ctrl.get_audio_capture_mode().unwrap(), AudioCaptureMode::MusicBalance);

    // 5. Filter Mode (XU2 0x06, 1 byte)
    ctrl.set_filter(FilterMode::Amber).unwrap();
    assert_eq!(ctrl.get_filter().unwrap(), FilterMode::Amber);

    // 6. Privacy Mode (XU2 0x08, 4 bytes) & Horizontal Correction (XU2 0x0d, 1 byte)
    ctrl.set_privacy_mode(true).unwrap();
    let (_, priv_enabled) = ctrl.get_privacy_mode().unwrap();
    assert!(priv_enabled);
    assert!(ctrl.get_device_status().unwrap().contains(DeviceStatus::PRIVACY_SLEEP));

    ctrl.set_horizontal_correction(false).unwrap();
    assert!(!ctrl.get_horizontal_correction().unwrap());

    // 7. Image Templates (XU2 0x04 & 0x05)
    ctrl.set_image_template(3, ImageOperation::SaveOrUpdate).unwrap();
    ctrl.set_image_template_name(3, "Podcast Look").unwrap();
    let tmpl_info = ctrl.get_image_template_info().unwrap();
    assert_eq!(tmpl_info.current_id, 3);
    assert_ne!(tmpl_info.bitmask & (1 << 3), 0);
    ctrl.delete_image_template(3).unwrap();
    assert_eq!(ctrl.get_image_template_info().unwrap().bitmask & (1 << 3), 0);

    // 8. Extended Function Bitmask (XU1 0x0a, 8 bytes: [mask_u32_le, values_u32_le])
    for &func in &ExtendFunction::ALL {
        ctrl.set_extend_function(func, true).unwrap();
        let statuses = ctrl.get_extend_function_status().unwrap();
        let is_on = statuses.into_iter().find(|(f, _)| *f == func).unwrap().1;
        assert!(is_on, "expected {:?} to be enabled", func);
    }

    // Verify specific wire payloads in command_log()
    let log = mock.command_log();
    // Find EV compensation command
    let ev_cmd = log
        .iter()
        .find(|(_, u, s, _)| *u == DEFAULT_EXTENSION_UNIT_ID && *s == xu1_selectors::EXPOSURE_COMPENSATION)
        .expect("EV compensation command logged");
    assert_eq!(ev_cmd.3, (-1.5f32).to_le_bytes().to_vec());

    // Find Privacy Mode command
    let priv_cmd = log
        .iter()
        .find(|(_, u, s, _)| *u == DEFAULT_EXTENSION_UNIT2_ID && *s == xu2_selectors::PRIVACY_AND_FILL_LIGHT)
        .expect("Privacy mode command logged");
    assert_eq!(priv_cmd.3, vec![0x02, 0x01, 0x00, 0x00]);

    // Find Filter Mode command
    let filter_cmd = log
        .iter()
        .find(|(_, u, s, _)| *u == DEFAULT_EXTENSION_UNIT2_ID && *s == xu2_selectors::FILTER_MODE)
        .expect("Filter mode command logged");
    assert_eq!(filter_cmd.3, vec![FilterMode::Amber as u8]);
}

#[test]
fn test_system_info_tracking_rects_and_mode_switch_commands() {
    let mock = MockCameraTransport::new(CameraModel::Link2);
    let ctrl = Insta360CameraController::new(mock.clone());

    // Device Info (XU2 0x03, 150 bytes)
    let dev_info = ctrl.get_device_info().unwrap();
    assert_eq!(dev_info.device_name, "Insta360 Link 2");
    assert_eq!(dev_info.firmware_version, "v2.5.8.8_build1");
    assert_eq!(dev_info.serial_number, "INSTA360SIM0001");

    // PTZ Version & Angles (XU1 0x0d, 30 bytes)
    ctrl.set_pantilt_absolute(3600, -7200).unwrap();
    let ptz_info = ctrl.get_ptz_version_and_angles().unwrap();
    assert_eq!(ptz_info.version, "v1.3.1.0_build0");
    assert_eq!(ptz_info.pan, 10);
    assert_eq!(ptz_info.tilt, -20);
    assert_eq!(ptz_info.zoom, 100);

    // Select Tracked Object (XU1 0x05, 10 bytes)
    let target = UvcRect {
        x: 100,
        y: 200,
        width: 300,
        height: 400,
        track_id: 7,
    };
    ctrl.select_track_object(&target).unwrap();

    // Default Track Mode (XU2 0x0c, 1 byte)
    ctrl.set_default_track_mode(1).unwrap();
    assert_eq!(ctrl.get_default_track_mode().unwrap(), 1);

    // System Timestamp, IQ Switch, Factory Restore (XU2 0x02, 10 bytes)
    ctrl.set_timestamp(0x0102_0304_0506_0708).unwrap();
    ctrl.set_iq_switch(1, true, 0x01).unwrap();
    ctrl.restore_factory_settings(true).unwrap();
    ctrl.restore_factory_settings(false).unwrap();

    // Factory Default (XU1 0x01 sub-opcode 4) & MSC Switch (XU1 0x0f sub-opcode 2)
    ctrl.load_factory_default().unwrap();
    ctrl.switch_msc_readonly_class().unwrap();

    let log = mock.command_log();
    assert!(log.iter().any(|(_, u, s, p)| {
        *u == DEFAULT_EXTENSION_UNIT_ID
            && *s == xu1_selectors::TRACK_OBJECT_SELECT
            && p == &encode_track_rect(&target)
    }));
    assert!(log.iter().any(|(_, u, s, p)| {
        *u == DEFAULT_EXTENSION_UNIT2_ID
            && *s == xu2_selectors::SYSTEM_TIME_AND_IQ
            && p[0] == 0x01
            && &p[1..9] == &0x0102_0304_0506_0708u64.to_le_bytes()
    }));
    assert!(log.iter().any(|(_, u, s, p)| {
        *u == DEFAULT_EXTENSION_UNIT2_ID
            && *s == xu2_selectors::SYSTEM_TIME_AND_IQ
            && p == &[0x02, 0x03, 0x00, 0x00, 0x00, 0, 0, 0, 0, 0]
    }));
    assert!(log.iter().any(|(_, u, s, p)| {
        *u == DEFAULT_EXTENSION_UNIT_ID
            && *s == xu1_selectors::USB_MODE_SWITCH
            && p == &[0x02, 0x00, 0x00, 0x00]
    }));

    // Gesture bindings (XU2 selector 0x01)
    ctrl.set_gesture_binding_func(Gesture::Palm, GestureFunction::AutoFraming)
        .unwrap();
    let bindings = ctrl.get_gesture_binding_map().unwrap();
    assert_eq!(bindings[1], (Gesture::Palm, GestureFunction::AutoFraming));

    // Tracking Forbidden Areas (XU1 selector 0x01 opcodes 7/8 + XU2 selector 0x0b)
    ctrl.enter_track_forbidden_area_setting().unwrap();
    let areas = vec![
        ForbiddenAreaInfo {
            enabled: true,
            index: 0,
            pan_min: -300,
            pan_max: -100,
            tilt_min: -150,
            tilt_max: 150,
        },
        ForbiddenAreaInfo {
            enabled: false,
            index: 1,
            pan_min: 100,
            pan_max: 300,
            tilt_min: -100,
            tilt_max: 100,
        },
    ];
    ctrl.set_track_forbidden_areas(&areas).unwrap();
    let read_areas = ctrl.get_track_forbidden_areas().unwrap();
    assert_eq!(read_areas, areas);
    ctrl.exit_track_forbidden_area_setting().unwrap();

    // Real zoom & Fill light param
    ctrl.set_zoom(250).unwrap();
    assert_eq!(ctrl.get_real_zoom_value().unwrap(), 250);
    assert_eq!(ctrl.get_fill_light_param().unwrap(), (0, 0));
}

