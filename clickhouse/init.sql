-- =====================================================
-- 古代铸钟工艺仿真与钟声传播模拟系统 - ClickHouse 初始化脚本
-- =====================================================

CREATE DATABASE IF NOT EXISTS bell_casting
    COMMENT '古代铸钟工艺仿真数据库'
    ENGINE = Atomic;

USE bell_casting;

-- =====================================================
-- 1. 钟体信息表
-- =====================================================
CREATE TABLE IF NOT EXISTS bells (
    bell_id UUID DEFAULT generateUUIDv4(),
    bell_name String COMMENT '钟名称，如曾侯乙编钟#3、永乐大钟',
    dynasty String COMMENT '朝代：先秦、汉代、唐代、明代等',
    bell_type String COMMENT '类型：编钟、朝钟、佛钟、永乐大钟',
    material String COMMENT '材质：青铜、黄铜、响铜',
    height_m Float64 COMMENT '高度(米)',
    diameter_m Float64 COMMENT '口径(米)',
    weight_kg Float64 COMMENT '重量(公斤)',
    expected_pitch String COMMENT '预期音高，如C4、G5',
    expected_freq_hz Float64 COMMENT '预期基频(Hz)',
    created_at DateTime DEFAULT now() COMMENT '创建时间',
    PRIMARY KEY (bell_id)
)
ENGINE = MergeTree()
ORDER BY (bell_id, created_at)
COMMENT '钟体基础信息';

-- =====================================================
-- 2. 传感器实时数据表（每小时上报）
-- =====================================================
CREATE TABLE IF NOT EXISTS sensor_readings (
    reading_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '采集时间',
    temp_celsius Float64 COMMENT '温度(摄氏度)',
    temp_gradient Float64 COMMENT '温度梯度(°C/m)',
    wall_thickness_mm Float64 COMMENT '壁厚(毫米)',
    thickness_deviation Float64 COMMENT '壁厚偏差(%)',
    alloy_cu Float64 COMMENT '铜含量(%)',
    alloy_sn Float64 COMMENT '锡含量(%)',
    alloy_pb Float64 COMMENT '铅含量(%)',
    alloy_zn Float64 COMMENT '锌含量(%)',
    alloy_other Float64 COMMENT '其他成分(%)',
    acoustic_freq_hz Float64 COMMENT '实测基频(Hz)',
    acoustic_amplitude Float64 COMMENT '振幅',
    acoustic_decay Float64 COMMENT '衰减系数',
    acoustic_harmonics Array(Float64) COMMENT '各次谐波频率',
    PRIMARY KEY (reading_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '传感器数据 - 合金成分、温度、壁厚、声学参数';

-- =====================================================
-- 3. 铸造仿真结果表（凝固过程、缩孔预测）
-- =====================================================
CREATE TABLE IF NOT EXISTS casting_simulation (
    sim_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '仿真时间',
    sim_type String COMMENT '仿真类型：solidification(凝固)、shrinkage(缩孔)、stress(应力)',
    time_step_sec UInt32 COMMENT '仿真时间步(秒)',
    temp_field Array(Array(Array(Float64))) COMMENT '3D温度场 [x][y][z]',
    solid_fraction Array(Array(Array(Float64))) COMMENT '固相率场 0~1',
    shrinkage_porosity Array(Array(Array(Float64))) COMMENT '缩孔率场 0~1',
    defect_locations Array(Tuple(Float64, Float64, Float64, Float64)) COMMENT '缺陷位置(x,y,z,严重度)',
    defect_count UInt32 COMMENT '缺陷总数',
    max_shrinkage Float64 COMMENT '最大缩孔率',
    cooling_rate Float64 COMMENT '冷却速率(°C/s)',
    prediction_risk String COMMENT '风险等级：low/medium/high/critical',
    PRIMARY KEY (sim_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, timestamp)
TTL timestamp + INTERVAL 6 MONTH
COMMENT '铸造仿真 - 凝固理论与缩孔预测';

-- =====================================================
-- 4. 声学仿真结果表（有限元法、声场云图）
-- =====================================================
CREATE TABLE IF NOT EXISTS acoustic_simulation (
    sim_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '仿真时间',
    method String COMMENT '计算方法：FEM(有限元)、BEM(边界元)',
    natural_frequencies Array(Float64) COMMENT '各阶固有频率(Hz)',
    mode_shapes Array(Array(Array(Array(Float64)))) COMMENT '各阶振型 [mode][x][y][z][disp]',
    far_field_pressure Array(Tuple(Float64, Float64, Float64)) COMMENT '远场声压 (theta, phi, pressure_dB)',
    sound_field_2d Array(Array(Float64)) COMMENT '2D声场截面云图数据',
    directivity_index Float64 COMMENT '指向性指数',
    sound_power Float64 COMMENT '辐射声功率(W)',
    pitch_deviation_cents Float64 COMMENT '音准偏差(音分)',
    pitch_ok Boolean COMMENT '音准是否合格',
    PRIMARY KEY (sim_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, timestamp)
TTL timestamp + INTERVAL 6 MONTH
COMMENT '声学仿真 - 固有频率、远场声压、声场云图';

-- =====================================================
-- 5. 告警事件表
-- =====================================================
CREATE TABLE IF NOT EXISTS alerts (
    alert_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '告警时间',
    alert_type String COMMENT '告警类型：defect(铸造缺陷)、pitch(音准偏差)、temp(温度异常)、alloy(成分异常)',
    severity String COMMENT '严重等级：warning、danger、critical',
    message String COMMENT '告警详情',
    related_reading UUID COMMENT '关联传感器读数ID',
    related_sim UUID COMMENT '关联仿真ID',
    resolved Boolean DEFAULT false COMMENT '是否已处理',
    resolved_at DateTime COMMENT '处理时间',
    PRIMARY KEY (alert_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, severity, timestamp)
TTL timestamp + INTERVAL 2 YEAR
COMMENT '告警事件表';

-- =====================================================
-- 6. 铸造过程状态表（用于前端动画）
-- =====================================================
CREATE TABLE IF NOT EXISTS casting_process (
    process_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '时间戳',
    stage String COMMENT '阶段：molding(制模)、melting(熔炼)、pouring(浇注)、cooling(冷却)、solidifying(凝固)、finished(完成)',
    progress Float64 COMMENT '进度 0~1',
    current_temp Float64 COMMENT '当前温度',
    mold_fill_level Float64 COMMENT '铸型填充率 0~1',
    PRIMARY KEY (process_id)
)
ENGINE = MergeTree()
ORDER BY (bell_id, timestamp)
COMMENT '铸造过程状态';

-- =====================================================
-- 数据视图 - 最新传感器数据
-- =====================================================
CREATE VIEW IF NOT EXISTS latest_sensor_data AS
SELECT
    bell_id,
    argMax(timestamp, timestamp) AS last_update,
    argMax(temp_celsius, timestamp) AS temp_celsius,
    argMax(wall_thickness_mm, timestamp) AS wall_thickness_mm,
    argMax(acoustic_freq_hz, timestamp) AS acoustic_freq_hz,
    argMax(acoustic_decay, timestamp) AS acoustic_decay
FROM sensor_readings
GROUP BY bell_id;

-- =====================================================
-- 数据视图 - 活跃告警
-- =====================================================
CREATE VIEW IF NOT EXISTS active_alerts AS
SELECT *
FROM alerts
WHERE resolved = false
ORDER BY timestamp DESC;

-- =====================================================
-- 示例数据
-- =====================================================
INSERT INTO bells (bell_name, dynasty, bell_type, material, height_m, diameter_m, weight_kg, expected_pitch, expected_freq_hz) VALUES
('曾侯乙编钟#1', '先秦', '编钟', '青铜', 0.75, 0.52, 28.5, 'C4', 261.63),
('曾侯乙编钟#2', '先秦', '编钟', '青铜', 0.68, 0.47, 22.3, 'D4', 293.66),
('曾侯乙编钟#3', '先秦', '编钟', '青铜', 0.82, 0.58, 38.7, 'E4', 329.63),
('永乐大钟', '明代', '朝钟', '响铜', 6.75, 3.30, 46500.0, 'A1', 55.00),
('寒山寺大钟', '明代', '佛钟', '青铜', 2.50, 1.80, 5800.0, 'G2', 98.00);
