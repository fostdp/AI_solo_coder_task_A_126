#!/usr/bin/env python3
"""
古代铸钟工艺仿真与钟声传播模拟系统 - 高级模拟器
功能：
1. 模拟每件钟每1小时上报传感器数据（合金成分、温度、壁厚、声学参数）
2. 支持自定义合金成分（铜、锡、铅、锌比例）
3. 支持自定义钟体形状（高度、直径、壁厚、钟口弧度）
4. 支持注入铸造缺陷（缩孔、裂纹、偏析、夹渣）
5. 支持注入声学参数异常（音准偏差、谐波失真、衰减异常）
6. 模拟铸造过程（制模→熔炼→浇注→冷却→凝固）
7. 调用后端铸造仿真和声学仿真接口
8. Daemon模式：持续运行，按间隔上报数据
9. 交互式配置：动态调整参数
"""

import argparse
import json
import math
import os
import random
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Tuple
from uuid import UUID

import requests

BACKEND_URL = os.environ.get("BACKEND_URL", "http://localhost:8080")

# ============================================================
# 合金配方库
# ============================================================
ALLOY_RECIPES: Dict[str, Dict[str, Tuple[float, float]]] = {
    "bronze_qing_qin": {
        "name": "先秦锡青铜（曾侯乙编钟）",
        "cu": (83.0, 86.0),
        "sn": (12.0, 14.0),
        "pb": (0.8, 2.0),
        "zn": (0.2, 0.6),
        "other": (0.5, 1.5),
    },
    "bronze_yong_le": {
        "name": "明代低锡青铜（永乐大钟）",
        "cu": (78.0, 82.0),
        "sn": (15.0, 18.0),
        "pb": (1.0, 2.5),
        "zn": (0.3, 1.0),
        "other": (0.5, 1.5),
    },
    "bronze_fo_zhong": {
        "name": "清代高锡青铜（佛钟）",
        "cu": (74.0, 78.0),
        "sn": (18.0, 21.0),
        "pb": (1.5, 3.0),
        "zn": (0.5, 1.5),
        "other": (0.5, 2.0),
    },
    "gray_cast_iron": {
        "name": "灰铸铁（近现代钟）",
        "cu": (0.0, 0.5),
        "sn": (0.0, 0.3),
        "pb": (94.0, 96.0),
        "zn": (2.5, 4.0),
        "other": (1.0, 2.0),
    },
    "high_tin_bronze": {
        "name": "高锡青铜（22%锡）",
        "cu": (75.0, 77.0),
        "sn": (21.0, 23.0),
        "pb": (0.5, 1.0),
        "zn": (0.2, 0.5),
        "other": (0.5, 1.0),
    },
}

# ============================================================
# 钟体形状模板
# ============================================================
BELL_SHAPE_TEMPLATES: Dict[str, Dict] = {
    "bianzhong_niu": {
        "name": "编钟（钮钟）",
        "height_to_diameter_ratio": 1.3,
        "wall_thickness_ratio": 0.035,
        "mouth_flare_angle": 5.0,
        "shape_factor": 0.85,
    },
    "bianzhong_yong": {
        "name": "编钟（甬钟）",
        "height_to_diameter_ratio": 1.5,
        "wall_thickness_ratio": 0.04,
        "mouth_flare_angle": 7.0,
        "shape_factor": 0.90,
    },
    "chao_zhong": {
        "name": "朝钟",
        "height_to_diameter_ratio": 2.0,
        "wall_thickness_ratio": 0.045,
        "mouth_flare_angle": 3.0,
        "shape_factor": 0.95,
    },
    "fo_zhong": {
        "name": "佛钟",
        "height_to_diameter_ratio": 1.4,
        "wall_thickness_ratio": 0.05,
        "mouth_flare_angle": 8.0,
        "shape_factor": 0.80,
    },
    "yong_le_da_zhong": {
        "name": "永乐大钟型",
        "height_to_diameter_ratio": 2.05,
        "wall_thickness_ratio": 0.048,
        "mouth_flare_angle": 4.0,
        "shape_factor": 0.92,
    },
}

# ============================================================
# 铸造阶段定义
# ============================================================
CASTING_STAGES = [
    ("molding", "制模", 3600, 25.0, 25.0, 0.0),
    ("melting", "熔炼", 7200, 25.0, 1200.0, 0.0),
    ("pouring", "浇注", 1800, 1200.0, 1150.0, 1.0),
    ("cooling", "冷却", 14400, 1150.0, 400.0, 1.0),
    ("solidifying", "凝固", 28800, 400.0, 80.0, 1.0),
    ("finished", "完成", 0, 25.0, 25.0, 1.0),
]

# ============================================================
# 缺陷类型
# ============================================================
DEFECT_TYPES = {
    "shrinkage_cavity": "缩孔",
    "shrinkage_porosity": "缩松",
    "crack_hot": "热裂纹",
    "crack_cold": "冷裂纹",
    "segregation": "成分偏析",
    "inclusion": "夹渣",
    "gas_porosity": "气孔",
    "cold_shut": "冷隔",
}


@dataclass
class AlloyConfig:
    """合金成分配置"""
    recipe: str = "bronze_yong_le"
    custom_cu: Optional[float] = None
    custom_sn: Optional[float] = None
    custom_pb: Optional[float] = None
    custom_zn: Optional[float] = None
    custom_other: Optional[float] = None

    def get_composition(self) -> Dict[str, float]:
        """获取合金成分（随机波动或自定义值）"""
        if self.recipe in ALLOY_RECIPES:
            base = ALLOY_RECIPES[self.recipe]
            result = {
                "alloy_cu": self.custom_cu if self.custom_cu else round(random.uniform(*base["cu"]), 2),
                "alloy_sn": self.custom_sn if self.custom_sn else round(random.uniform(*base["sn"]), 2),
                "alloy_pb": self.custom_pb if self.custom_pb else round(random.uniform(*base["pb"]), 2),
                "alloy_zn": self.custom_zn if self.custom_zn else round(random.uniform(*base["zn"]), 2),
                "alloy_other": self.custom_other if self.custom_other else round(random.uniform(*base["other"]), 2),
            }
        else:
            result = {
                "alloy_cu": self.custom_cu or 80.0,
                "alloy_sn": self.custom_sn or 15.0,
                "alloy_pb": self.custom_pb or 2.0,
                "alloy_zn": self.custom_zn or 1.0,
                "alloy_other": self.custom_other or 2.0,
            }
        return result


@dataclass
class ShapeConfig:
    """钟体形状配置"""
    template: str = "yong_le_da_zhong"
    height_m: Optional[float] = None
    diameter_m: Optional[float] = None
    wall_thickness_mm: Optional[float] = None
    mouth_flare_angle: Optional[float] = None

    def get_dimensions(self, bell: Dict) -> Tuple[float, float, float]:
        """获取尺寸（高度m, 直径m, 壁厚mm）"""
        height = self.height_m or bell.get("height_m", 1.0)
        diameter = self.diameter_m or bell.get("diameter_m", 0.7)

        if self.wall_thickness_mm:
            thickness = self.wall_thickness_mm
        else:
            if self.template in BELL_SHAPE_TEMPLATES:
                ratio = BELL_SHAPE_TEMPLATES[self.template]["wall_thickness_ratio"]
                thickness = diameter * 1000 * ratio
            else:
                thickness = diameter * 1000 * 0.04

        return height, diameter, thickness


@dataclass
class DefectInjection:
    """缺陷注入配置"""
    enabled: bool = False
    defect_type: str = "shrinkage_cavity"
    severity: str = "medium"
    defect_count: int = 3
    custom_defects: List[Dict] = field(default_factory=list)

    SEVERITY_MAP = {
        "low": (1, 3, 0.01, 0.05),
        "medium": (3, 8, 0.05, 0.15),
        "high": (8, 20, 0.15, 0.30),
        "critical": (20, 50, 0.30, 0.50),
    }

    def apply_to_reading(self, reading: Dict) -> Dict:
        """将缺陷效果应用到传感器读数"""
        if not self.enabled:
            return reading

        if self.severity in self.SEVERITY_MAP:
            min_count, max_count, min_size, max_size = self.SEVERITY_MAP[self.severity]
        else:
            min_count, max_count, min_size, max_size = self.SEVERITY_MAP["medium"]

        if self.defect_type in ("shrinkage_cavity", "shrinkage_porosity"):
            reading["wall_thickness_mm"] *= random.uniform(0.85, 0.95)
            reading["thickness_deviation"] = max(
                reading["thickness_deviation"],
                random.uniform(8, 25),
            )
        elif self.defect_type in ("crack_hot", "crack_cold"):
            reading["acoustic_decay"] *= random.uniform(1.3, 2.0)
            reading["acoustic_amplitude"] *= random.uniform(0.6, 0.8)
        elif self.defect_type == "segregation":
            reading["alloy_sn"] += random.uniform(2, 5)
            reading["alloy_pb"] += random.uniform(1, 3)
        elif self.defect_type == "gas_porosity":
            reading["acoustic_freq_hz"] *= random.uniform(0.95, 0.98)
            reading["acoustic_decay"] *= random.uniform(1.1, 1.4)
        elif self.defect_type == "inclusion":
            reading["acoustic_amplitude"] *= random.uniform(0.7, 0.9)

        return reading


@dataclass
class AcousticInjection:
    """声学参数注入配置"""
    enabled: bool = False
    pitch_shift_cents: float = 0.0
    harmonic_distortion: float = 0.0
    decay_factor: float = 1.0
    amplitude_factor: float = 1.0
    custom_freq: Optional[float] = None
    custom_harmonics: Optional[List[float]] = None

    def apply_to_reading(self, reading: Dict) -> Dict:
        """将声学效果应用到传感器读数"""
        if not self.enabled:
            return reading

        base_freq = reading["acoustic_freq_hz"]

        if self.custom_freq:
            reading["acoustic_freq_hz"] = self.custom_freq
        elif self.pitch_shift_cents != 0:
            reading["acoustic_freq_hz"] = base_freq * (2 ** (self.pitch_shift_cents / 1200))

        if self.custom_harmonics:
            reading["acoustic_harmonics"] = self.custom_harmonics
        elif self.harmonic_distortion > 0:
            harmonics = reading.get("acoustic_harmonics", [])
            distorted = []
            for i, h in enumerate(harmonics):
                distortion = h * random.uniform(
                    -self.harmonic_distortion,
                    self.harmonic_distortion,
                )
                distorted.append(h + distortion)
            reading["acoustic_harmonics"] = distorted

        reading["acoustic_amplitude"] *= self.amplitude_factor
        reading["acoustic_decay"] *= self.decay_factor

        return reading


class BellSimulator:
    """高级铸钟模拟器"""

    def __init__(self, backend_url: str = BACKEND_URL):
        self.backend_url = backend_url.rstrip("/")
        self.session = requests.Session()
        self.bells = self._fetch_bells()
        self.alloy_config = AlloyConfig()
        self.shape_config = ShapeConfig()
        self.defect_injection = DefectInjection()
        self.acoustic_injection = AcousticInjection()

    def _fetch_bells(self) -> List[Dict]:
        try:
            resp = self.session.get(f"{self.backend_url}/bells", timeout=10)
            resp.raise_for_status()
            data = resp.json()
            if data.get("success"):
                return data.get("data", [])
        except Exception as e:
            print(f"[警告] 获取钟列表失败: {e}")
        return []

    def list_alloy_recipes(self) -> List[Dict]:
        return [{"key": k, "name": v["name"]} for k, v in ALLOY_RECIPES.items()]

    def list_shape_templates(self) -> List[Dict]:
        return [{"key": k, "name": v["name"]} for k, v in BELL_SHAPE_TEMPLATES.items()]

    def list_defect_types(self) -> List[Dict]:
        return [{"key": k, "name": v} for k, v in DEFECT_TYPES.items()]

    def set_alloy_recipe(self, recipe: str):
        if recipe in ALLOY_RECIPES:
            self.alloy_config.recipe = recipe
            print(f"[配置] 合金配方设置为: {ALLOY_RECIPES[recipe]['name']}")
        else:
            print(f"[错误] 未知合金配方: {recipe}")

    def set_custom_alloy(self, **kwargs):
        for key in ["cu", "sn", "pb", "zn", "other"]:
            if key in kwargs and kwargs[key] is not None:
                setattr(self.alloy_config, f"custom_{key}", kwargs[key])
        print("[配置] 自定义合金成分已更新")

    def set_shape_template(self, template: str):
        if template in BELL_SHAPE_TEMPLATES:
            self.shape_config.template = template
            print(f"[配置] 形状模板设置为: {BELL_SHAPE_TEMPLATES[template]['name']}")
        else:
            print(f"[错误] 未知形状模板: {template}")

    def set_custom_shape(self, height_m=None, diameter_m=None, wall_thickness_mm=None):
        if height_m:
            self.shape_config.height_m = height_m
        if diameter_m:
            self.shape_config.diameter_m = diameter_m
        if wall_thickness_mm:
            self.shape_config.wall_thickness_mm = wall_thickness_mm
        print("[配置] 自定义形状参数已更新")

    def enable_defect(self, defect_type: str, severity: str = "medium", count: int = 3):
        if defect_type not in DEFECT_TYPES:
            print(f"[错误] 未知缺陷类型: {defect_type}")
            return
        self.defect_injection.enabled = True
        self.defect_injection.defect_type = defect_type
        self.defect_injection.severity = severity
        self.defect_injection.defect_count = count
        print(f"[配置] 缺陷注入已启用: {DEFECT_TYPES[defect_type]} ({severity}, {count}个)")

    def disable_defect(self):
        self.defect_injection.enabled = False
        print("[配置] 缺陷注入已禁用")

    def enable_acoustic_injection(self, **kwargs):
        self.acoustic_injection.enabled = True
        for key, value in kwargs.items():
            if hasattr(self.acoustic_injection, key) and value is not None:
                setattr(self.acoustic_injection, key, value)
        print("[配置] 声学参数注入已启用")

    def disable_acoustic_injection(self):
        self.acoustic_injection.enabled = False
        print("[配置] 声学参数注入已禁用")

    def _generate_temperature(self, hour_in_cycle: int, inject_error: bool = False) -> Tuple[float, float]:
        T_POUR = 1180
        T_AMBIENT = 25
        tau_cool = 8.0

        if hour_in_cycle < 2:
            temp = T_POUR + random.uniform(-20, 20)
            gradient = random.uniform(80, 120)
        elif hour_in_cycle < 24:
            t = hour_in_cycle
            temp = T_AMBIENT + (T_POUR - T_AMBIENT) * math.exp(-t / tau_cool)
            temp += random.uniform(-15, 15)
            gradient = (100 * math.exp(-t / tau_cool)) + random.uniform(-10, 10)
        else:
            temp = T_AMBIENT + random.uniform(-5, 5)
            gradient = random.uniform(0, 5)

        if inject_error:
            temp = T_POUR + random.uniform(30, 80)
            gradient = random.uniform(150, 200)

        return round(temp, 2), round(gradient, 2)

    def generate_sensor_reading(
        self,
        bell: Dict,
        hour_in_cycle: int,
        inject_alloy_error: bool = False,
        inject_temp_error: bool = False,
        inject_thickness_error: bool = False,
        inject_acoustic_error: bool = False,
    ) -> Dict:
        alloy = self.alloy_config.get_composition()
        temp, temp_grad = self._generate_temperature(hour_in_cycle, inject_temp_error)

        _, _, base_thickness = self.shape_config.get_dimensions(bell)
        if inject_thickness_error:
            deviation_pct = random.choice([random.uniform(-30, -15), random.uniform(15, 30)])
        else:
            deviation_pct = random.uniform(-8, 8)
        thickness = base_thickness * (1 + deviation_pct / 100)

        expected_freq = bell.get("expected_freq_hz", 261.63)
        if inject_acoustic_error:
            freq_error_cents = random.choice([random.uniform(-200, -80), random.uniform(80, 200)])
            freq = expected_freq * (2 ** (freq_error_cents / 1200))
        else:
            freq_error_cents = random.uniform(-40, 40)
            freq = expected_freq * (2 ** (freq_error_cents / 1200))

        harmonics = [freq * (2 + i * 0.5 + random.uniform(-0.05, 0.05)) for i in range(6)]
        amplitude = random.uniform(0.6, 1.0)
        decay = random.uniform(0.3, 0.8)

        if inject_acoustic_error:
            amplitude = random.uniform(0.2, 0.4)
            decay = random.uniform(1.2, 2.0)

        reading = {
            "bell_id": bell["bell_id"],
            "temp_celsius": temp,
            "temp_gradient": temp_grad,
            "wall_thickness_mm": round(thickness, 2),
            "thickness_deviation": round(deviation_pct, 2),
            **alloy,
            "acoustic_freq_hz": round(freq, 4),
            "acoustic_amplitude": round(amplitude, 4),
            "acoustic_decay": round(decay, 4),
            "acoustic_harmonics": [round(h, 4) for h in harmonics],
        }

        reading = self.defect_injection.apply_to_reading(reading)
        reading = self.acoustic_injection.apply_to_reading(reading)

        return reading

    def send_sensor_reading(self, reading: Dict) -> Optional[Dict]:
        try:
            resp = self.session.post(
                f"{self.backend_url}/sensors",
                json=reading,
                timeout=10,
            )
            resp.raise_for_status()
            data = resp.json()
            if data.get("success"):
                return data.get("data")
            else:
                print(f"[错误] 发送传感器数据失败: {data.get('error')}")
        except Exception as e:
            print(f"[错误] 发送传感器数据异常: {e}")
        return None

    def simulate_casting_process(self, bell: Dict, accelerated: bool = False):
        print(f"\n=== 开始模拟铸造过程: {bell['bell_name']} ===")
        bell_id = bell["bell_id"]

        for stage_name, stage_cn, duration_sec, temp_start, temp_end, fill_end in CASTING_STAGES:
            if stage_name == "finished":
                process = {
                    "bell_id": bell_id,
                    "stage": stage_name,
                    "progress": 1.0,
                    "current_temp": temp_end,
                    "mold_fill_level": fill_end,
                }
                self.session.post(f"{self.backend_url}/casting-process", json=process)
                print(f"  [{stage_cn:6s}] 完成 100%")
                continue

            steps = 20 if accelerated else 100
            step_duration = duration_sec / steps / (100 if accelerated else 1)

            for step in range(1, steps + 1):
                progress = step / steps
                current_temp = temp_start + (temp_end - temp_start) * progress
                fill_level = min(fill_end, progress * fill_end) if stage_name == "pouring" else fill_end

                process = {
                    "bell_id": bell_id,
                    "stage": stage_name,
                    "progress": round(progress, 4),
                    "current_temp": round(current_temp, 2),
                    "mold_fill_level": round(fill_level, 4),
                }
                self.session.post(f"{self.backend_url}/casting-process", json=process)

                if step % 10 == 0 or step == steps:
                    print(
                        f"  [{stage_cn:6s}] {progress*100:5.1f}% | "
                        f"温度: {current_temp:7.1f}°C | 填充: {fill_level*100:5.1f}%"
                    )

                if accelerated:
                    time.sleep(0.02)
                else:
                    time.sleep(max(0.1, step_duration / 10))

        print(f"=== 铸造完成: {bell['bell_name']} ===")

    def run_casting_simulation(
        self,
        bell: Dict,
        initial_temp: Optional[float] = None,
        sim_type: str = "niyama",
        grid_size: int = 20,
    ) -> Optional[Dict]:
        if initial_temp is None:
            initial_temp = random.uniform(1100, 1200)

        payload = {
            "bell_id": bell["bell_id"],
            "sim_type": sim_type,
            "initial_temp": initial_temp,
            "grid_size": grid_size,
        }
        try:
            resp = self.session.post(
                f"{self.backend_url}/sim/casting",
                json=payload,
                timeout=60,
            )
            resp.raise_for_status()
            data = resp.json()
            if data.get("success"):
                sim = data["data"]
                print(
                    f"  [铸造仿真] {bell['bell_name']} | "
                    f"风险: {sim['prediction_risk']:8s} | "
                    f"最大缩孔率: {sim['max_shrinkage']*100:5.2f}% | "
                    f"缺陷数: {sim['defect_count']:2d}"
                )
                return sim
        except Exception as e:
            print(f"[错误] 铸造仿真失败: {e}")
        return None

    def run_acoustic_simulation(
        self,
        bell: Dict,
        method: str = "BEM",
    ) -> Optional[Dict]:
        payload = {
            "bell_id": bell["bell_id"],
            "method": method,
            "young_modulus": 1.1e11,
            "poisson_ratio": 0.34,
            "density": 8800.0,
        }
        try:
            resp = self.session.post(
                f"{self.backend_url}/sim/acoustic",
                json=payload,
                timeout=60,
            )
            resp.raise_for_status()
            data = resp.json()
            if data.get("success"):
                sim = data["data"]
                status = "合格" if sim["pitch_ok"] else "偏差"
                print(
                    f"  [声学仿真] {bell['bell_name']} | "
                    f"音准: {status} ({sim['pitch_deviation_cents']:+.1f}音分) | "
                    f"声功率: {sim['sound_power']:.4f}W"
                )
                return sim
        except Exception as e:
            print(f"[错误] 声学仿真失败: {e}")
        return None

    def run_hourly_simulation(
        self,
        total_hours: int = 72,
        error_rate: float = 0.05,
        sleep_sec: float = 1.0,
    ):
        print(f"开始每小时模拟，共 {total_hours} 小时，异常注入率 {error_rate*100:.0f}%")
        print(f"可用钟体数量: {len(self.bells)}")
        for b in self.bells:
            print(f"  - {b['bell_name']} ({b['dynasty']}, {b['bell_type']})")

        if self.defect_injection.enabled:
            print(f"[注入] 缺陷: {DEFECT_TYPES[self.defect_injection.defect_type]} ({self.defect_injection.severity})")
        if self.acoustic_injection.enabled:
            print(f"[注入] 声学: 音移{self.acoustic_injection.pitch_shift_cents}音分")

        for hour in range(total_hours):
            print(f"\n{'='*60}")
            print(f"第 {hour+1}/{total_hours} 小时 ({datetime.now().strftime('%H:%M:%S')})")
            print("=" * 60)

            for bell in self.bells:
                r = random.random()
                inject_alloy = r < error_rate / 4
                inject_temp = r < error_rate / 2
                inject_thickness = r < error_rate * 0.75
                inject_acoustic = r < error_rate

                reading = self.generate_sensor_reading(
                    bell,
                    hour_in_cycle=hour,
                    inject_alloy_error=inject_alloy,
                    inject_temp_error=inject_temp,
                    inject_thickness_error=inject_thickness,
                    inject_acoustic_error=inject_acoustic,
                )

                result = self.send_sensor_reading(reading)
                alerts = result.get("alerts_triggered", 0) if result else 0
                status_flags = []
                if inject_alloy:
                    status_flags.append("成分")
                if inject_temp:
                    status_flags.append("温度")
                if inject_thickness:
                    status_flags.append("壁厚")
                if inject_acoustic:
                    status_flags.append("音准")
                flag_str = f" [异常:{','.join(status_flags)}]" if status_flags else ""
                alert_str = f" -> {alerts}告警" if alerts > 0 else ""

                print(
                    f"  {bell['bell_name']:12s} | "
                    f"T={reading['temp_celsius']:7.1f}°C | "
                    f"f={reading['acoustic_freq_hz']:7.2f}Hz | "
                    f"厚度={reading['wall_thickness_mm']:6.2f}mm"
                    f"{flag_str}{alert_str}"
                )

            if hour % 6 == 0 and hour > 0:
                print(f"\n--- 每6小时触发仿真 ---")
                for bell in self.bells:
                    self.run_casting_simulation(bell)
                    self.run_acoustic_simulation(bell)

            time.sleep(sleep_sec)

    def run_daemon(self, interval_sec: int = 60, error_rate: float = 0.05):
        """守护进程模式：持续运行，周期性上报"""
        print(f"[守护模式] 启动，间隔 {interval_sec}秒，异常率 {error_rate*100:.0f}%")
        print("=" * 60)

        hour_counter = 0
        while True:
            try:
                print(f"\n[{datetime.now().strftime('%Y-%m-%d %H:%M:%S')}] 第 {hour_counter+1} 轮上报")
                for bell in self.bells:
                    r = random.random()
                    reading = self.generate_sensor_reading(
                        bell,
                        hour_in_cycle=hour_counter % 72,
                        inject_alloy_error=r < error_rate / 4,
                        inject_temp_error=r < error_rate / 2,
                        inject_thickness_error=r < error_rate * 0.75,
                        inject_acoustic_error=r < error_rate,
                    )
                    self.send_sensor_reading(reading)

                if hour_counter % 6 == 0:
                    for bell in self.bells:
                        self.run_casting_simulation(bell)
                        self.run_acoustic_simulation(bell)

                hour_counter += 1
                time.sleep(interval_sec)
            except KeyboardInterrupt:
                print("\n[守护模式] 收到中断信号，退出")
                break
            except Exception as e:
                print(f"[错误] 运行异常: {e}")
                time.sleep(interval_sec)


def interactive_mode(sim: BellSimulator):
    """交互式配置模式"""
    print("\n=== 交互式配置模式 ===")
    print("输入 'help' 查看命令，输入 'exit' 退出")

    while True:
        try:
            cmd = input("\n[模拟器] > ").strip().lower()
        except (EOFError, KeyboardInterrupt):
            print("\n退出交互模式")
            break

        if not cmd:
            continue

        parts = cmd.split()
        action = parts[0]

        if action in ("exit", "quit", "q"):
            break
        elif action == "help":
            print("""
可用命令:
  bells                   - 列出所有钟体
  alloys                  - 列出合金配方
  shapes                  - 列出形状模板
  defects                 - 列出缺陷类型
  set-alloy <配方名>      - 设置合金配方
  custom-alloy Cu Sn Pb Zn Other - 设置自定义合金成分
  set-shape <模板名>      - 设置形状模板
  custom-shape h=... d=... t=... - 自定义形状(高度/直径/壁厚mm)
  enable-defect <类型> [严重度] [数量] - 启用缺陷注入
  disable-defect          - 禁用缺陷注入
  enable-acoustic pitch=... decay=... amp=... - 启用声学注入
  disable-acoustic        - 禁声学注入
  status                  - 查看当前配置
  run <小时数> [错误率]   - 运行模拟
  casting <钟名>          - 运行铸造过程模拟
  sim-casting <钟名>      - 运行铸造仿真
  sim-acoustic <钟名>     - 运行声学仿真
  daemon [间隔秒]         - 守护进程模式
""")
        elif action == "bells":
            for i, b in enumerate(sim.bells):
                print(f"  [{i}] {b['bell_name']} ({b['dynasty']}, {b['bell_type']})")
        elif action == "alloys":
            for a in sim.list_alloy_recipes():
                print(f"  {a['key']:20s} - {a['name']}")
        elif action == "shapes":
            for s in sim.list_shape_templates():
                print(f"  {s['key']:20s} - {s['name']}")
        elif action == "defects":
            for d in sim.list_defect_types():
                print(f"  {d['key']:25s} - {d['name']}")
        elif action == "set-alloy" and len(parts) >= 2:
            sim.set_alloy_recipe(parts[1])
        elif action == "custom-alloy" and len(parts) >= 5:
            try:
                sim.set_custom_alloy(
                    cu=float(parts[1]),
                    sn=float(parts[2]),
                    pb=float(parts[3]),
                    zn=float(parts[4]),
                    other=float(parts[5]) if len(parts) >= 6 else None,
                )
            except ValueError as e:
                print(f"参数错误: {e}")
        elif action == "set-shape" and len(parts) >= 2:
            sim.set_shape_template(parts[1])
        elif action == "custom-shape":
            params = {}
            for p in parts[1:]:
                if "=" in p:
                    k, v = p.split("=", 1)
                    params[k] = float(v)
            sim.set_custom_shape(
                height_m=params.get("h"),
                diameter_m=params.get("d"),
                wall_thickness_mm=params.get("t"),
            )
        elif action == "enable-defect" and len(parts) >= 2:
            severity = parts[2] if len(parts) >= 3 else "medium"
            count = int(parts[3]) if len(parts) >= 4 else 3
            sim.enable_defect(parts[1], severity, count)
        elif action == "disable-defect":
            sim.disable_defect()
        elif action == "enable-acoustic":
            params = {}
            for p in parts[1:]:
                if "=" in p:
                    k, v = p.split("=", 1)
                    params[k] = float(v)
            sim.enable_acoustic_injection(**params)
        elif action == "disable-acoustic":
            sim.disable_acoustic_injection()
        elif action == "status":
            print(f"  合金配方: {sim.alloy_config.recipe}")
            print(f"  形状模板: {sim.shape_config.template}")
            print(f"  缺陷注入: {'启用' if sim.defect_injection.enabled else '禁用'}")
            if sim.defect_injection.enabled:
                print(f"    类型: {DEFECT_TYPES[sim.defect_injection.defect_type]}")
                print(f"    严重度: {sim.defect_injection.severity}")
            print(f"  声学注入: {'启用' if sim.acoustic_injection.enabled else '禁用'}")
            if sim.acoustic_injection.enabled:
                print(f"    音移: {sim.acoustic_injection.pitch_shift_cents}音分")
        elif action == "run":
            hours = int(parts[1]) if len(parts) >= 2 else 24
            error_rate = float(parts[2]) if len(parts) >= 3 else 0.05
            sim.run_hourly_simulation(total_hours=hours, error_rate=error_rate, sleep_sec=0.5)
        elif action == "casting" and len(parts) >= 2:
            bell_name = parts[1]
            bell = next((b for b in sim.bells if bell_name in b["bell_name"]), None)
            if bell:
                sim.simulate_casting_process(bell, accelerated=True)
            else:
                print(f"未找到钟: {bell_name}")
        elif action == "sim-casting" and len(parts) >= 2:
            bell_name = parts[1]
            bell = next((b for b in sim.bells if bell_name in b["bell_name"]), None)
            if bell:
                sim.run_casting_simulation(bell)
            else:
                print(f"未找到钟: {bell_name}")
        elif action == "sim-acoustic" and len(parts) >= 2:
            bell_name = parts[1]
            bell = next((b for b in sim.bells if bell_name in b["bell_name"]), None)
            if bell:
                sim.run_acoustic_simulation(bell)
            else:
                print(f"未找到钟: {bell_name}")
        elif action == "daemon":
            interval = int(parts[1]) if len(parts) >= 2 else 60
            sim.run_daemon(interval_sec=interval)
        else:
            print(f"未知命令: {cmd}。输入 'help' 查看帮助")


def main():
    parser = argparse.ArgumentParser(
        description="古代铸钟工艺高级模拟器 - 支持自定义合金、形状、缺陷注入",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  python bell_simulator.py --interactive           # 交互式模式
  python bell_simulator.py --mode daemon           # 守护进程模式
  python bell_simulator.py --alloy bronze_qing_qin # 使用先秦青铜配方
  python bell_simulator.py --defect shrinkage_cavity --severity high  # 注入缩孔缺陷
  python bell_simulator.py --pitch-shift -50       # 音准降低50音分
  python bell_simulator.py --hours 72 --error-rate 0.1  # 运行72小时，10%异常率
""",
    )
    parser.add_argument(
        "--backend", default=BACKEND_URL, help=f"后端URL (默认: {BACKEND_URL})"
    )
    parser.add_argument("--hours", type=int, default=24, help="模拟小时数")
    parser.add_argument("--error-rate", type=float, default=0.05, help="异常数据注入率 (0-1)")
    parser.add_argument("--sleep", type=float, default=1.0, help="每小时间隔秒数")

    # 合金配置
    parser.add_argument("--alloy", default=None, help="合金配方名 (如 bronze_yong_le)")
    parser.add_argument("--cu", type=float, default=None, help="自定义铜含量(%%)")
    parser.add_argument("--sn", type=float, default=None, help="自定义锡含量(%%)")
    parser.add_argument("--pb", type=float, default=None, help="自定义铅含量(%%)")
    parser.add_argument("--zn", type=float, default=None, help="自定义锌含量(%%)")

    # 形状配置
    parser.add_argument("--shape", default=None, help="形状模板名 (如 yong_le_da_zhong)")
    parser.add_argument("--height", type=float, default=None, help="自定义高度(m)")
    parser.add_argument("--diameter", type=float, default=None, help="自定义直径(m)")
    parser.add_argument("--thickness", type=float, default=None, help="自定义壁厚(mm)")

    # 缺陷注入
    parser.add_argument("--defect", default=None, help="缺陷类型 (如 shrinkage_cavity)")
    parser.add_argument("--severity", default="medium", help="缺陷严重度: low/medium/high/critical")
    parser.add_argument("--defect-count", type=int, default=3, help="缺陷数量")

    # 声学注入
    parser.add_argument("--pitch-shift", type=float, default=0.0, help="音准偏移(音分)")
    parser.add_argument("--harmonic-distortion", type=float, default=0.0, help="谐波失真率")
    parser.add_argument("--decay-factor", type=float, default=1.0, help="衰减倍率")
    parser.add_argument("--amplitude-factor", type=float, default=1.0, help="振幅倍率")

    # 运行模式
    parser.add_argument("--casting", action="store_true", help="执行铸造过程模拟")
    parser.add_argument("--sim-casting", action="store_true", help="立即运行铸造仿真")
    parser.add_argument("--sim-acoustic", action="store_true", help="立即运行声学仿真")
    parser.add_argument("--accelerated", action="store_true", help="加速模式")
    parser.add_argument("--once", action="store_true", help="只运行一轮后退出")
    parser.add_argument("--interactive", action="store_true", help="交互式模式")
    parser.add_argument("--mode", default="hourly", choices=["hourly", "daemon", "once"], help="运行模式")
    parser.add_argument("--interval", type=int, default=60, help="守护模式间隔秒数")

    args = parser.parse_args()

    print("=" * 60)
    print("古代铸钟工艺高级模拟器")
    print("=" * 60)

    sim = BellSimulator(args.backend)
    if not sim.bells:
        print("[错误] 没有可用钟体，请检查后端服务是否启动")
        sys.exit(1)

    # 应用配置
    if args.alloy:
        sim.set_alloy_recipe(args.alloy)
    if args.cu or args.sn or args.pb or args.zn:
        sim.set_custom_alloy(cu=args.cu, sn=args.sn, pb=args.pb, zn=args.zn)
    if args.shape:
        sim.set_shape_template(args.shape)
    if args.height or args.diameter or args.thickness:
        sim.set_custom_shape(
            height_m=args.height,
            diameter_m=args.diameter,
            wall_thickness_mm=args.thickness,
        )
    if args.defect:
        sim.enable_defect(args.defect, args.severity, args.defect_count)
    if args.pitch_shift or args.harmonic_distortion or args.decay_factor != 1.0 or args.amplitude_factor != 1.0:
        sim.enable_acoustic_injection(
            pitch_shift_cents=args.pitch_shift,
            harmonic_distortion=args.harmonic_distortion,
            decay_factor=args.decay_factor,
            amplitude_factor=args.amplitude_factor,
        )

    # 交互式模式
    if args.interactive:
        interactive_mode(sim)
        return

    # 铸造过程模拟
    if args.casting:
        for bell in sim.bells:
            sim.simulate_casting_process(bell, accelerated=args.accelerated)

    # 立即仿真
    if args.sim_casting:
        print("\n=== 铸造仿真 ===")
        for bell in sim.bells:
            sim.run_casting_simulation(bell)
    if args.sim_acoustic:
        print("\n=== 声学仿真 ===")
        for bell in sim.bells:
            sim.run_acoustic_simulation(bell)

    # 运行模式
    if args.mode == "daemon":
        sim.run_daemon(interval_sec=args.interval, error_rate=args.error_rate)
    elif args.mode == "once" or args.once:
        sim.run_hourly_simulation(total_hours=1, error_rate=args.error_rate, sleep_sec=0)
    else:
        if not (args.casting and not args.once and not (args.sim_casting or args.sim_acoustic)):
            sim.run_hourly_simulation(
                total_hours=args.hours,
                error_rate=args.error_rate,
                sleep_sec=args.sleep,
            )

    print("\n模拟完成。")


if __name__ == "__main__":
    main()
