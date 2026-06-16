# 古代铸钟工艺仿真与钟声传播模拟系统

> 面向音乐考古研究的全栈应用，模拟先秦编钟、明代永乐大钟等古代铸钟的铸造工艺与钟声传播特性。

[![Rust](https://img.shields.io/badge/Rust-1.77+-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![ClickHouse](https://img.shields.io/badge/ClickHouse-24.3-yellow?logo=clickhouse&logoColor=white)](https://clickhouse.com/)
[![MQTT](https://img.shields.io/badge/MQTT-5.0-00a2e8?logo=eclipse-mosquitto&logoColor=white)](https://mosquitto.org/)
[![Docker](https://img.shields.io/badge/Docker-Compose-2496ed?logo=docker&logoColor=white)](https://www.docker.com/)
[![Three.js](https://img.shields.io/badge/Three.js-r120+-black?logo=three.js&logoColor=white)](https://threejs.org/)

---

## 🏛️ 项目背景

本系统为音乐考古团队研究古代铸钟工艺提供数字化仿真平台，支持：

- **铸造工艺仿真**：基于凝固理论与Niyama判据，预测缩孔、缩松等铸造缺陷
- **声学特性仿真**：基于边界元法(BEM)，计算钟体固有频率与远场声压分布
- **实时监测告警**：传感器数据采集 + MQTT告警推送
- **三维可视化**：钟体三维模型、铸造过程动画、声场云图

---

## 🏗️ 系统架构

### 整体架构图

```
┌──────────────────────────────────────────────────────────────────┐
│                        前端 (Nginx + Gzip)                       │
│  ┌──────────────┐       ┌──────────────┐     ┌───────────────┐  │
│  │  bell_3d.js  │       │acoustic_panel│     │   app.js      │  │
│  │  3D铸钟渲染  │◄──────┤  声场可视化  │◄────┤  应用协调层   │  │
│  └──────────────┘       └──────────────┘     └───────┬───────┘  │
│                                                       │          │
└───────────────────────────────────────────────────────┼──────────┘
                                                        │
┌─────────────────────────── /api/ ─────────────────────┼──────────┐
│                                                        ▼          │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                Rust Backend (微服务架构)                │    │
│  │  ┌───────────┐    ┌───────────────┐    ┌────────────┐  │    │
│  │  │ dtu_      │───►│ casting_      │───►│ alarm_     │  │    │
│  │  │ receiver  │    │ simulator     │    │ mqtt       │  │    │
│  │  └────┬──────┘    └───────────────┘    └─────┬──────┘  │    │
│  │       │       ┌──────────────────┐            │         │    │
│  │       └──────►│ acoustic_        │────────────┘         │    │
│  │               │ simulator        │                      │    │
│  │               └──────────────────┘                      │    │
│  │                                                          │    │
│  │  Tokio mpsc channels  •  tracing  •  Prometheus metrics │    │
│  └──────────────────────────┬───────────────────────────────┘    │
│                             │                                    │
└─────────────────────────────┼────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
  ┌─────────────┐      ┌─────────────┐     ┌─────────────┐
  │ ClickHouse  │      │   Mosquitto │     │  Simulator  │
  │ 时序数据库  │      │  MQTT Broker│     │  (Python)   │
  │ • 降采样    │      │  告警推送    │     │ • 合金配置  │
  │ • TTL保留   │      │             │     │ • 形状配置  │
  │ • 物化视图  │      │             │     │ • 缺陷注入  │
  └─────────────┘      └─────────────┘     │ • 声学注入  │
                                            └─────────────┘
```

### 微服务模块说明

| 模块 | 文件 | 职责 |
|------|------|------|
| **dtu_receiver** | [dtu_receiver.rs](backend/src/dtu_receiver.rs) | 传感器数据采集、12项物理校验、消息分发 |
| **casting_simulator** | [casting_simulator.rs](backend/src/casting_simulator.rs) | 凝固仿真、Niyama缩孔判据、缺陷预测 |
| **acoustic_simulator** | [acoustic_simulator.rs](backend/src/acoustic_simulator.rs) | BEM声学仿真、Tikhonov正则化、音准评估 |
| **alarm_mqtt** | [alarm_mqtt.rs](backend/src/alarm_mqtt.rs) | 4类告警检测、MQTT推送 |
| **消息总线** | [message_bus.rs](backend/src/message_bus.rs) | 8种消息类型、12种通道别名 |

### 前端模块说明

| 模块 | 文件 | 职责 |
|------|------|------|
| **Bell3D** | [bell_3d.js](frontend/bell_3d.js) | Three.js 3D渲染、LatheGeometry铸钟模型、铸造动画、缺陷可视化 |
| **AcousticPanel** | [acoustic_panel.js](frontend/acoustic_panel.js) | WebWorker声场计算、Canvas 2D渲染、频谱条形图 |

---

## 📁 目录结构

```
.
├── backend/                    # Rust后端
│   ├── src/
│   │   ├── simulation/         # 仿真引擎 (铸造+声学)
│   │   ├── dtu_receiver.rs     # 数据采集模块
│   │   ├── casting_simulator.rs# 铸造仿真模块
│   │   ├── acoustic_simulator.rs# 声学仿真模块
│   │   ├── alarm_mqtt.rs       # 告警MQTT模块
│   │   ├── metrics_collector.rs# Prometheus指标
│   │   ├── config_loader.rs    # 配置加载
│   │   ├── message_bus.rs      # 消息总线
│   │   ├── handlers.rs         # HTTP API
│   │   ├── db.rs               # ClickHouse存储
│   │   └── main.rs             # 入口
│   ├── Cargo.toml
│   └── Dockerfile              # 多阶段构建 + 静态二进制
├── frontend/                   # 前端静态资源
│   ├── index.html
│   ├── app.js
│   ├── bell_3d.js
│   ├── acoustic_panel.js
│   └── sound_field_worker.js
├── simulator/                  # 铸钟工艺模拟器
│   ├── bell_simulator.py
│   ├── requirements.txt
│   └── Dockerfile
├── clickhouse/                 # 数据库初始化
│   └── init.sql
├── config/                     # 配置文件
│   ├── materials.json          # 材料参数配置
│   ├── acoustic_params.json    # 声学参数配置
│   ├── .env.example            # 环境变量示例
│   ├── nginx.conf              # Nginx配置 (Gzip+缓存)
│   ├── mosquitto.conf          # MQTT配置
│   ├── prometheus.yml          # Prometheus配置
│   ├── clickhouse-config.xml   # ClickHouse配置
│   └── grafana/                # Grafana面板
├── docker-compose.yml          # 容器编排
└── README.md
```

---

## 🚀 快速部署

### 环境要求

- Docker >= 24.0
- Docker Compose >= 2.20
- 至少 4GB 内存 (推荐 8GB)

### 一键启动

```bash
# 1. 克隆项目
git clone <repo-url>
cd bell-casting-simulator

# 2. 复制环境变量配置
cp config/.env.example .env

# 3. 启动核心服务 (后端 + ClickHouse + MQTT + 前端)
docker-compose up -d

# 4. 启动模拟器 (可选)
docker-compose --profile simulator up -d

# 5. 启动监控 (可选，含Prometheus + Grafana)
docker-compose --profile monitoring up -d

# 6. 查看服务状态
docker-compose ps
```

### 服务访问地址

| 服务 | 地址 | 默认端口 |
|------|------|----------|
| 前端 | http://localhost:8081 | 8081 |
| 后端API | http://localhost:8080 | 8080 |
| Metrics | http://localhost:9090/metrics | 9090 |
| ClickHouse | http://localhost:8123 | 8123 |
| MQTT | mqtt://localhost:1883 | 1883 |
| Prometheus | http://localhost:9091 | 9091 |
| Grafana | http://localhost:3000 | 3000 |

### 本地开发模式

```bash
# 后端 (Rust)
cd backend
cargo run --release

# 前端 (Python简单HTTP服务器)
cd frontend
python -m http.server 8081

# 模拟器
cd simulator
pip install -r requirements.txt
python bell_simulator.py --interactive
```

---

## 🔧 铸钟工艺模拟器使用

### 功能特性

- ✅ **5种合金配方**：先秦锡青铜、明代低锡青铜、清代高锡青铜、灰铸铁、高锡青铜
- ✅ **5种形状模板**：编钟(钮钟/甬钟)、朝钟、佛钟、永乐大钟型
- ✅ **8种缺陷注入**：缩孔、缩松、热裂纹、冷裂纹、成分偏析、夹渣、气孔、冷隔
- ✅ **声学参数定制**：音准偏移、谐波失真、衰减/振幅倍率
- ✅ **3种运行模式**：每小时模拟、守护进程、交互式配置
- ✅ **铸造过程模拟**：制模→熔炼→浇注→冷却→凝固→完成

### 命令行用法

```bash
cd simulator
pip install -r requirements.txt

# 查看帮助
python bell_simulator.py --help

# 交互式模式 (推荐)
python bell_simulator.py --interactive

# 守护进程模式，30秒上报一次
python bell_simulator.py --mode daemon --interval 30

# 使用先秦青铜配方，注入缩孔缺陷(严重度high)
python bell_simulator.py \
  --alloy bronze_qing_qin \
  --defect shrinkage_cavity \
  --severity high

# 音准降低50音分，谐波失真10%
python bell_simulator.py \
  --pitch-shift -50 \
  --harmonic-distortion 0.1

# 自定义合金成分: 铜78% 锡18% 铅2% 锌1%
python bell_simulator.py --cu 78 --sn 18 --pb 2 --zn 1

# 自定义形状: 高度2m 直径1.2m 壁厚50mm
python bell_simulator.py --height 2 --diameter 1.2 --thickness 50

# 运行72小时模拟，10%异常率
python bell_simulator.py --hours 72 --error-rate 0.1
```

### 交互式命令

```
[模拟器] > help

可用命令:
  bells                   - 列出所有钟体
  alloys                  - 列出合金配方
  shapes                  - 列出形状模板
  defects                 - 列出缺陷类型
  set-alloy <配方名>      - 设置合金配方
  custom-alloy Cu Sn Pb Zn [Other] - 自定义合金成分
  set-shape <模板名>      - 设置形状模板
  custom-shape h=... d=... t=...   - 自定义形状(高/直径/壁厚mm)
  enable-defect <类型> [严重度] [数量] - 启用缺陷注入
  disable-defect          - 禁用缺陷注入
  enable-acoustic pitch=... decay=... amp=... - 启用声学注入
  disable-acoustic        - 禁用声学注入
  status                  - 查看当前配置
  run <小时数> [错误率]   - 运行模拟
  casting <钟名>          - 运行铸造过程模拟
  sim-casting <钟名>      - 运行铸造仿真
  sim-acoustic <钟名>     - 运行声学仿真
  daemon [间隔秒]         - 守护进程模式
```

### 缺陷类型说明

| 类型 | 名称 | 影响 |
|------|------|------|
| `shrinkage_cavity` | 缩孔 | 壁厚减薄、壁厚偏差增大 |
| `shrinkage_porosity` | 缩松 | 壁厚均匀性下降 |
| `crack_hot` | 热裂纹 | 声学衰减增大、振幅降低 |
| `crack_cold` | 冷裂纹 | 声学衰减显著增大 |
| `segregation` | 成分偏析 | 锡/铅含量异常升高 |
| `gas_porosity` | 气孔 | 频率降低、衰减增大 |
| `inclusion` | 夹渣 | 振幅降低 |
| `cold_shut` | 冷隔 | 综合声学性能下降 |

### Docker中运行模拟器

```bash
# 运行一次性模拟
docker-compose run simulator --hours 24 --error-rate 0.1

# 交互模式
docker-compose run simulator --interactive

# 守护进程 (随系统启动)
docker-compose --profile simulator up -d
```

---

## 📊 监控与可观测性

### Prometheus 指标

后端暴露 `/metrics` 端点，提供以下指标：

| 指标 | 类型 | 说明 |
|------|------|------|
| `bell_sensor_readings_total` | Counter | 总传感器读数 |
| `bell_sensor_readings_valid` | Counter | 有效读数 |
| `bell_sensor_readings_invalid` | Counter | 无效读数 |
| `bell_casting_sim_total` | Counter | 铸造仿真次数 |
| `bell_acoustic_sim_total` | Counter | 声学仿真次数 |
| `bell_alerts_total` | Counter | 告警总数 (按类型/严重度分标签) |
| `bell_mqtt_published_total` | Counter | MQTT发布成功数 |
| `bell_mqtt_failed_total` | Counter | MQTT发布失败数 |
| `bell_active_bells` | Gauge | 活跃钟体数 |
| `bell_active_alerts` | Gauge | 活跃告警数 |
| `bell_casting_sim_duration_seconds` | Histogram | 铸造仿真耗时 |
| `bell_acoustic_sim_duration_seconds` | Histogram | 声学仿真耗时 |
| `bell_request_duration_seconds` | Histogram | HTTP请求耗时 |

访问地址: http://localhost:9090/metrics

### Grafana 面板

启动监控profile后访问 http://localhost:3000 (默认账号: admin/admin)

预设面板：
- 系统总览：传感器数据量、仿真次数、告警趋势
- 铸造仿真：缩孔率、缺陷数、冷却速率分布
- 声学特性：固有频率分布、音准偏差、声功率
- 告警统计：按类型/严重度/钟体统计

---

## 💾 数据保留策略

### ClickHouse 数据分级存储

| 表 | 粒度 | 保留时长 | 说明 |
|----|------|----------|------|
| `sensor_readings` | 原始(每小时) | 30天 | 原始传感器数据，TTL自动清理 |
| `sensor_readings_hourly` | 小时级 | 6个月 | 物化视图自动聚合，SummingMergeTree |
| `sensor_readings_daily` | 日级 | 3年 | 小时级进一步聚合，长期分析用 |
| `casting_simulation` | 仿真结果 | 3个月 | 铸造仿真原始数据 |
| `acoustic_simulation` | 仿真结果 | 3个月 | 声学仿真原始数据 |
| `alerts` | 告警事件 | 1年 | 告警历史记录 |
| `casting_process` | 过程状态 | 1个月 | 铸造动画过程数据 |

### 物化视图链路

```
sensor_readings (原始)
    │
    └──► sensor_readings_to_hourly (MV)
            │
            └──► sensor_readings_hourly (小时级)
                    │
                    └──► sensor_readings_to_daily (MV)
                            │
                            └──► sensor_readings_daily (日级)
```

---

## 🔌 API 接口

### 传感器数据

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/sensors` | 上报传感器读数 |
| GET | `/sensors/bell/:bell_id` | 获取某钟的传感器历史 |
| GET | `/sensors/latest` | 获取所有钟最新数据 |

### 仿真接口

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/sim/casting` | 提交铸造仿真 |
| POST | `/sim/acoustic` | 提交声学仿真 |
| GET | `/sim/casting/:bell_id` | 获取铸造仿真历史 |
| GET | `/sim/acoustic/:bell_id` | 获取声学仿真历史 |

### 钟体管理

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/bells` | 获取所有钟体 |
| GET | `/bells/:id` | 获取钟体详情 |
| POST | `/bells` | 新增钟体 |

### 告警接口

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/alerts` | 获取告警列表 |
| GET | `/alerts/active` | 获取活跃告警 |
| POST | `/alerts/:id/resolve` | 标记告警已处理 |

### 健康检查

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/metrics` | Prometheus指标 |

---

## 🎯 技术栈

### 后端
- **语言**: Rust 2021 Edition
- **框架**: Axum + Tokio (异步runtime)
- **数据库**: ClickHouse 24.3 (MergeTree + 物化视图 + TTL)
- **消息队列**: MQTT (rumqttc)
- **仿真引擎**: 铸造(Niyama判据+Scheil方程)、声学(BEM+Tikhonov正则化)
- **监控**: metrics crate + Prometheus exporter
- **日志**: tracing + tracing-subscriber
- **部署**: Docker多阶段构建 + scratch镜像 + 静态二进制

### 前端
- **3D渲染**: Three.js + LatheGeometry
- **声场可视化**: Canvas 2D + WebWorker
- **样式**: 原生CSS
- **部署**: Nginx + Gzip压缩 + 静态资源缓存

### 运维
- **编排**: Docker Compose
- **监控**: Prometheus + Grafana
- **数据库**: ClickHouse原生监控端点

---

## 📚 相关文档

- [铸造仿真模型](backend/src/simulation/casting.rs) - 凝固理论、Niyama判据
- [声学仿真模型](backend/src/simulation/acoustic.rs) - BEM边界元、Tikhonov正则化
- [材料配置](config/materials.json) - 4种材料物理参数
- [声学配置](config/acoustic_params.json) - 6大类声学参数
- [告警阈值](config/acoustic_params.json) - 温度/壁厚/音准告警配置

---

## 🤝 贡献指南

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

---

## 📄 许可证

本项目采用 MIT 许可证 - 详见 LICENSE 文件

---

## 📧 联系

如有问题或建议，欢迎提交 Issue 或 PR。

---

*古代铸钟工艺仿真与钟声传播模拟系统 - 让千年古钟重获新声*
