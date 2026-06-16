-- =====================================================
-- 古代铸钟工艺仿真与钟声传播模拟系统 - ClickHouse 初始化脚本
-- 包含：基础表、降采样物化视图、保留策略
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
-- 2. 传感器实时数据表（每小时上报）- 原始数据
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
    acoustic_harmonics String COMMENT '各次谐波频率(JSON数组)',
    PRIMARY KEY (reading_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, timestamp)
TTL timestamp + INTERVAL 30 DAY DELETE
COMMENT '传感器原始数据 - 保留30天';

-- =====================================================
-- 2.1 传感器数据 - 小时级降采样
-- =====================================================
CREATE TABLE IF NOT EXISTS sensor_readings_hourly (
    bell_id UUID,
    timestamp_hour DateTime,
    temp_avg Float64,
    temp_min Float64,
    temp_max Float64,
    temp_gradient_avg Float64,
    wall_thickness_avg Float64,
    thickness_deviation_avg Float64,
    alloy_cu_avg Float64,
    alloy_sn_avg Float64,
    alloy_pb_avg Float64,
    alloy_zn_avg Float64,
    acoustic_freq_avg Float64,
    acoustic_amplitude_avg Float64,
    acoustic_decay_avg Float64,
    readings_count UInt64,
    PRIMARY KEY (bell_id, timestamp_hour)
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp_hour)
ORDER BY (bell_id, timestamp_hour)
TTL timestamp_hour + INTERVAL 6 MONTH DELETE
COMMENT '传感器小时级降采样 - 保留6个月';

-- =====================================================
-- 2.2 传感器数据 - 日级降采样
-- =====================================================
CREATE TABLE IF NOT EXISTS sensor_readings_daily (
    bell_id UUID,
    timestamp_date Date,
    temp_avg Float64,
    temp_min Float64,
    temp_max Float64,
    temp_gradient_avg Float64,
    wall_thickness_avg Float64,
    thickness_deviation_avg Float64,
    alloy_cu_avg Float64,
    alloy_sn_avg Float64,
    alloy_pb_avg Float64,
    alloy_zn_avg Float64,
    acoustic_freq_avg Float64,
    acoustic_amplitude_avg Float64,
    acoustic_decay_avg Float64,
    readings_count UInt64,
    PRIMARY KEY (bell_id, timestamp_date)
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp_date)
ORDER BY (bell_id, timestamp_date)
TTL timestamp_date + INTERVAL 3 YEAR DELETE
COMMENT '传感器日级降采样 - 保留3年';

-- =====================================================
-- 2.3 物化视图：原始数据 -> 小时级降采样
-- =====================================================
CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_readings_to_hourly
TO sensor_readings_hourly
AS
SELECT
    bell_id,
    toStartOfHour(timestamp) AS timestamp_hour,
    avg(temp_celsius) AS temp_avg,
    min(temp_celsius) AS temp_min,
    max(temp_celsius) AS temp_max,
    avg(temp_gradient) AS temp_gradient_avg,
    avg(wall_thickness_mm) AS wall_thickness_avg,
    avg(thickness_deviation) AS thickness_deviation_avg,
    avg(alloy_cu) AS alloy_cu_avg,
    avg(alloy_sn) AS alloy_sn_avg,
    avg(alloy_pb) AS alloy_pb_avg,
    avg(alloy_zn) AS alloy_zn_avg,
    avg(acoustic_freq_hz) AS acoustic_freq_avg,
    avg(acoustic_amplitude) AS acoustic_amplitude_avg,
    avg(acoustic_decay) AS acoustic_decay_avg,
    count() AS readings_count
FROM sensor_readings
GROUP BY bell_id, timestamp_hour;

-- =====================================================
-- 2.4 物化视图：小时级 -> 日级降采样
-- =====================================================
CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_readings_to_daily
TO sensor_readings_daily
AS
SELECT
    bell_id,
    toDate(timestamp_hour) AS timestamp_date,
    sum(temp_avg * readings_count) / sum(readings_count) AS temp_avg,
    min(temp_min) AS temp_min,
    max(temp_max) AS temp_max,
    sum(temp_gradient_avg * readings_count) / sum(readings_count) AS temp_gradient_avg,
    sum(wall_thickness_avg * readings_count) / sum(readings_count) AS wall_thickness_avg,
    sum(thickness_deviation_avg * readings_count) / sum(readings_count) AS thickness_deviation_avg,
    sum(alloy_cu_avg * readings_count) / sum(readings_count) AS alloy_cu_avg,
    sum(alloy_sn_avg * readings_count) / sum(readings_count) AS alloy_sn_avg,
    sum(alloy_pb_avg * readings_count) / sum(readings_count) AS alloy_pb_avg,
    sum(alloy_zn_avg * readings_count) / sum(readings_count) AS alloy_zn_avg,
    sum(acoustic_freq_avg * readings_count) / sum(readings_count) AS acoustic_freq_avg,
    sum(acoustic_amplitude_avg * readings_count) / sum(readings_count) AS acoustic_amplitude_avg,
    sum(acoustic_decay_avg * readings_count) / sum(readings_count) AS acoustic_decay_avg,
    sum(readings_count) AS readings_count
FROM sensor_readings_hourly
GROUP BY bell_id, timestamp_date;

-- =====================================================
-- 3. 铸造仿真结果表
-- =====================================================
CREATE TABLE IF NOT EXISTS casting_simulation (
    sim_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '仿真时间',
    sim_type String COMMENT '仿真类型：solidification、shrinkage、stress',
    time_step_sec UInt32 COMMENT '仿真时间步(秒)',
    temp_field String COMMENT '3D温度场 JSON',
    solid_fraction String COMMENT '固相率场 JSON',
    shrinkage_porosity String COMMENT '缩孔率场 JSON',
    defect_locations String COMMENT '缺陷位置 JSON',
    defect_count UInt32 COMMENT '缺陷总数',
    max_shrinkage Float64 COMMENT '最大缩孔率',
    cooling_rate Float64 COMMENT '冷却速率(°C/s)',
    prediction_risk String COMMENT '风险等级：low/medium/high/critical',
    PRIMARY KEY (sim_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, timestamp)
TTL timestamp + INTERVAL 3 MONTH DELETE
COMMENT '铸造仿真结果 - 保留3个月';

-- =====================================================
-- 4. 声学仿真结果表
-- =====================================================
CREATE TABLE IF NOT EXISTS acoustic_simulation (
    sim_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '仿真时间',
    method String COMMENT '计算方法：FEM、BEM',
    natural_frequencies String COMMENT '各阶固有频率 JSON',
    mode_shapes String COMMENT '各阶振型 JSON',
    far_field_pressure String COMMENT '远场声压 JSON',
    sound_field_2d String COMMENT '2D声场截面 JSON',
    directivity_index Float64 COMMENT '指向性指数',
    sound_power Float64 COMMENT '辐射声功率(W)',
    pitch_deviation_cents Float64 COMMENT '音准偏差(音分)',
    pitch_ok Boolean COMMENT '音准是否合格',
    PRIMARY KEY (sim_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, timestamp)
TTL timestamp + INTERVAL 3 MONTH DELETE
COMMENT '声学仿真结果 - 保留3个月';

-- =====================================================
-- 5. 告警事件表
-- =====================================================
CREATE TABLE IF NOT EXISTS alerts (
    alert_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '告警时间',
    alert_type String COMMENT '告警类型：defect、pitch、temp、alloy',
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
TTL timestamp + INTERVAL 1 YEAR DELETE
COMMENT '告警事件表 - 保留1年';

-- =====================================================
-- 6. 铸造过程状态表
-- =====================================================
CREATE TABLE IF NOT EXISTS casting_process (
    process_id UUID DEFAULT generateUUIDv4(),
    bell_id UUID COMMENT '关联钟ID',
    timestamp DateTime DEFAULT now() COMMENT '时间戳',
    stage String COMMENT '阶段：molding、melting、pouring、cooling、solidifying、finished',
    progress Float64 COMMENT '进度 0~1',
    current_temp Float64 COMMENT '当前温度',
    mold_fill_level Float64 COMMENT '铸型填充率 0~1',
    PRIMARY KEY (process_id)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (bell_id, timestamp)
TTL timestamp + INTERVAL 1 MONTH DELETE
COMMENT '铸造过程状态 - 保留1个月';

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
-- 数据视图 - 告警统计（按类型和严重程度）
-- =====================================================
CREATE VIEW IF NOT EXISTS alert_statistics AS
SELECT
    toStartOfDay(timestamp) AS date,
    alert_type,
    severity,
    count() AS alert_count,
    uniq(bell_id) AS affected_bells
FROM alerts
WHERE timestamp >= now() - INTERVAL 7 DAY
GROUP BY date, alert_type, severity
ORDER BY date DESC, alert_type, severity;

-- =====================================================
-- 示例数据
-- =====================================================
INSERT INTO bells (bell_name, dynasty, bell_type, material, height_m, diameter_m, weight_kg, expected_pitch, expected_freq_hz) VALUES
('曾侯乙编钟#1', '先秦', '编钟', '青铜', 0.75, 0.52, 28.5, 'C4', 261.63),
('曾侯乙编钟#2', '先秦', '编钟', '青铜', 0.68, 0.47, 22.3, 'D4', 293.66),
('曾侯乙编钟#3', '先秦', '编钟', '青铜', 0.82, 0.58, 38.7, 'E4', 329.63),
('永乐大钟', '明代', '朝钟', '响铜', 6.75, 3.30, 46500.0, 'A1', 55.00),
('寒山寺大钟', '明代', '佛钟', '青铜', 2.50, 1.80, 5800.0, 'G2', 98.00);
