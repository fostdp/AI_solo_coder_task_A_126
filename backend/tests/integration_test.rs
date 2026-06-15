use bell_casting_backend::config_loader::*;
use bell_casting_backend::message_bus::*;
use bell_casting_backend::models::*;
use chrono::Utc;
use uuid::Uuid;

#[tokio::test]
async fn test_config_loading() {
    println!("=== 测试1: 配置加载 ===");
    println!("材料配置数量: {}", MATERIALS.len());
    assert!(MATERIALS.len() > 0, "材料配置不能为空");
    
    let bronze = get_material("bronze_qing_qin");
    assert!(bronze.is_some(), "先秦锡青铜配置必须存在");
    let bronze = bronze.unwrap();
    println!("先秦锡青铜 - 密度: {} kg/m³, 杨氏模量: {} Pa", bronze.density, bronze.young_modulus);
    assert!(bronze.density > 0.0);
    assert!(bronze.young_modulus > 0.0);
    
    println!("声学配置 - BEM默认面板数: {}, 低频阈值: {}Hz", 
        ACOUSTIC_PARAMS.bem_solver.default_panels,
        ACOUSTIC_PARAMS.bem_solver.low_frequency_threshold_hz
    );
    assert!(ACOUSTIC_PARAMS.bem_solver.default_panels > 0);
    println!("✅ 配置加载测试通过");
}

#[tokio::test]
async fn test_message_bus() {
    println!("\n=== 测试2: 消息总线 ===");
    let (tx, mut rx) = tokio::sync::mpsc::channel::<BusMessage>(10);
    
    let reading = SensorReading {
        reading_id: Uuid::new_v4(),
        bell_id: Uuid::new_v4(),
        timestamp: Utc::now(),
        temp_celsius: 900.0,
        temp_gradient: 50.0,
        wall_thickness_mm: 100.0,
        thickness_deviation: 0.0,
        alloy_cu: 75.0,
        alloy_sn: 15.0,
        alloy_pb: 5.0,
        alloy_zn: 3.0,
        alloy_other: 2.0,
        acoustic_freq_hz: 261.63,
        acoustic_amplitude: 1.0,
        acoustic_decay: 2.0,
        acoustic_harmonics: "[1.0, 2.01, 3.02]".to_string(),
    };
    
    let bell = Bell {
        bell_id: Uuid::new_v4(),
        bell_name: "测试编钟".to_string(),
        dynasty: "先秦".to_string(),
        bell_type: "编钟".to_string(),
        material: "bronze_qing_qin".to_string(),
        height_m: 1.0,
        diameter_m: 0.7,
        weight_kg: 100.0,
        expected_pitch: "C4".to_string(),
        expected_freq_hz: 261.63,
        created_at: Utc::now(),
    };
    
    let msg = BusMessage::SensorReadingReceived {
        reading: reading.clone(),
        bell: Some(bell.clone()),
    };
    
    tx.send(msg).await.unwrap();
    
    let received = rx.recv().await.unwrap();
    match received {
        BusMessage::SensorReadingReceived { reading: r, bell: b } => {
            assert_eq!(r.temp_celsius, 900.0);
            assert_eq!(b.unwrap().bell_name, "测试编钟");
            println!("✅ 消息总线测试通过");
        }
        _ => panic!("收到错误的消息类型"),
    }
}

#[test]
fn test_casting_simulation() {
    println!("\n=== 测试3: 铸造仿真 ===");
    use bell_casting_backend::simulation::casting::simulate_casting;
    
    let material = get_default_material();
    let req = CastingSimRequest {
        bell_id: Uuid::new_v4(),
        sim_type: "niyama".to_string(),
        initial_temp: 1200.0,
        grid_size: Some(10),
    };
    
    let sim = simulate_casting(&req, material);
    
    println!("仿真ID: {}", sim.sim_id);
    println!("缺陷数量: {}", sim.defect_count);
    println!("最大缩孔率: {:.4}", sim.max_shrinkage);
    println!("冷却速率: {:.2} °C/s", sim.cooling_rate);
    println!("风险等级: {}", sim.prediction_risk);
    
    assert!(sim.defect_count > 0, "应该检测到一些缺陷");
    assert!(sim.max_shrinkage > 0.0, "最大缩孔率应该大于0");
    assert!(!sim.temp_field.is_empty(), "温度场JSON不应为空");
    println!("✅ 铸造仿真测试通过");
}

#[test]
fn test_acoustic_simulation() {
    println!("\n=== 测试4: 声学仿真 ===");
    use bell_casting_backend::simulation::acoustic::simulate_acoustic;
    
    let material = get_default_material();
    let req = AcousticSimRequest {
        bell_id: Uuid::new_v4(),
        method: "bem".to_string(),
        young_modulus: None,
        poisson_ratio: None,
        density: None,
    };
    
    let bell = Bell {
        bell_id: Uuid::new_v4(),
        bell_name: "测试钟".to_string(),
        dynasty: "明代".to_string(),
        bell_type: "朝钟".to_string(),
        material: "bronze_yong_le".to_string(),
        height_m: 2.0,
        diameter_m: 1.5,
        weight_kg: 500.0,
        expected_pitch: "C3".to_string(),
        expected_freq_hz: 130.81,
        created_at: Utc::now(),
    };
    
    let sim = simulate_acoustic(&req, Some(&bell), material);
    
    println!("仿真ID: {}", sim.sim_id);
    println!("基频偏差: {:.2} 音分", sim.pitch_deviation_cents);
    println!("音准合格: {}", sim.pitch_ok);
    println!("声功率: {:.2} W", sim.sound_power);
    println!("指向性指数: {:.2}", sim.directivity_index);
    
    assert!(!sim.natural_frequencies.is_empty(), "固有频率JSON不应为空");
    assert!(sim.sound_power > 0.0, "声功率应该大于0");
    println!("✅ 声学仿真测试通过");
}

#[test]
fn test_alert_thresholds() {
    println!("\n=== 测试5: 告警阈值 ===");
    
    let thresholds = &ACOUSTIC_PARAMS.alert_thresholds;
    println!("温度上限: {}°C", thresholds.temperature_max_celsius);
    println!("壁厚偏差警告: {}%", thresholds.thickness_deviation_warning_pct);
    println!("壁厚偏差危险: {}%", thresholds.thickness_deviation_danger_pct);
    println!("音准公差: {} 音分", ACOUSTIC_PARAMS.bell_acoustics.pitch_tolerance_cents);
    
    assert!(thresholds.temperature_max_celsius > 0.0);
    assert!(ACOUSTIC_PARAMS.bell_acoustics.pitch_tolerance_cents > 0.0);
    
    let test_cases = [
        (25.0, 261.63, 261.63, false),
        (900.0, 261.63, 261.63, true),
        (25.0, 261.63, 270.0, true),
    ];
    
    for (temp, expected, actual, should_alert) in test_cases {
        let temp_alert = temp > thresholds.temperature_max_celsius;
        let pitch_cents = 1200.0 * (actual / expected).log2();
        let pitch_alert = pitch_cents.abs() > ACOUSTIC_PARAMS.bell_acoustics.pitch_tolerance_cents;
        let has_alert = temp_alert || pitch_alert;
        println!("  温度:{temp}°C, 期望:{expected}Hz, 实际:{actual}Hz, 偏差:{pitch_cents:.1}音分, 告警:{has_alert} (预期:{should_alert})");
        assert_eq!(has_alert, should_alert);
    }
    println!("✅ 告警阈值测试通过");
}

#[test]
fn test_material_selection() {
    println!("\n=== 测试6: 材料选择 ===");
    
    let test_cases = [
        ("战国编钟", "bronze_qing_qin"),
        ("永乐大钟", "bronze_yong_le"),
        ("清代佛钟", "bronze_fo_zhong"),
        ("未知类型", "bronze_qing_qin"),
    ];
    
    for (bell_type, expected_key) in test_cases {
        let material = get_material_for_bell(bell_type);
        let expected = get_material(expected_key).unwrap_or_else(get_default_material);
        println!("  {bell_type} -> {}", material.name);
        assert_eq!(material.name, expected.name);
    }
    println!("✅ 材料选择测试通过");
}

fn main() {
    println!("========================================");
    println!("古代铸钟工艺仿真系统 - 功能回归测试");
    println!("========================================");
    
    // 运行所有测试
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    rt.block_on(test_config_loading());
    rt.block_on(test_message_bus());
    test_casting_simulation();
    test_acoustic_simulation();
    test_alert_thresholds();
    test_material_selection();
    
    println!("\n========================================");
    println!("✅ 所有功能回归测试通过！");
    println!("========================================");
}
