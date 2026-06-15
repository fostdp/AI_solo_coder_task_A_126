#!/usr/bin/env python3
"""
古代铸钟工艺仿真与钟声传播模拟系统 - 模拟器脚本
功能：
1. 模拟每件钟每1小时上报传感器数据（合金成分、温度、壁厚、声学参数）
2. 模拟铸造过程（制模→熔炼→浇注→冷却→凝固）
3. 调用后端铸造仿真和声学仿真接口
4. 注入异常数据触发告警
"""

import argparse
import json
import math
import random
import sys
import time
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Tuple
from uuid import UUID

import requests

BACKEND_URL = "http://localhost:8080"

# 先秦编钟标准合金比例：铜84.6%、锡12.6%、铅1.4%、其他1.4%
# 明代大钟：铜80.1%、锡16.4%、铅1.8%、锌0.6%、其他1.1%
BELL_ALLOY_STANDARDS = {
    "bianzhong": {  # 编钟
        "cu": (82.0, 87.0),
        "sn": (11.0, 14.0),
        "pb": (1.0, 3.0),
        "zn": (0.2, 0.8),
        "other": (0.5, 2.0),
    },
    "yongle": {  # 永乐大钟类型
        "cu": (78.0, 82.0),
        "sn": (15.0, 18.0),
        "pb": (1.0, 2.5),
        "zn": (0.3, 1.0),
        "other": (0.5, 2.0),
    },
    "fozhong": {  # 佛钟
        "cu": (75.0, 80.0),
        "sn": (16.0, 20.0),
        "pb": (1.5, 3.0),
        "zn": (0.5, 1.5),
        "other": (0.5, 2.0),
    },
}

# 铸造阶段
CASTING_STAGES = [
    ("molding", 3600, 25.0, 0.0, 0.0),
    ("melting", 7200, 25.0, 1200.0, 0.0),
    ("pouring", 1800, 1200.0, 1150.0, 1.0),
    ("cooling", 14400, 1150.0, 400.0, 1.0),
    ("solidifying", 28800, 400.0, 80.0, 1.0),
    ("finished", 0, 25.0, 25.0, 1.0),
]


class BellSimulator:
    def __init__(self, backend_url: str = BACKEND_URL):
        self.backend_url = backend_url.rstrip("/")
        self.session = requests.Session()
        self.bells = self._fetch_bells()

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

    def _get_alloy_type(self, bell: Dict) -> str:
        bell_type = bell.get("bell_type", "")
        if "编钟" in bell_type:
            return "bianzhong"
        elif "朝钟" in bell_type or bell.get("bell_name", "").startswith("永乐"):
            return "yongle"
        else:
            return "fozhong"

    def _generate_alloy(self, alloy_type: str, inject_error: bool = False) -> Dict:
        std = BELL_ALLOY_STANDARDS[alloy_type]
        if inject_error:
            anomaly = random.choice(["low_sn", "high_pb", "unbalanced"])
            if anomaly == "low_sn":
                std = dict(std)
                std["sn"] = (5.0, 8.0)
            elif anomaly == "high_pb":
                std = dict(std)
                std["pb"] = (6.0, 10.0)
            elif anomaly == "unbalanced":
                return {
                    "alloy_cu": round(random.uniform(60, 70), 2),
                    "alloy_sn": round(random.uniform(20, 25), 2),
                    "alloy_pb": round(random.uniform(5, 8), 2),
                    "alloy_zn": round(random.uniform(2, 4), 2),
                    "alloy_other": round(random.uniform(5, 10), 2),
                }

        return {
            "alloy_cu": round(random.uniform(*std["cu"]), 2),
            "alloy_sn": round(random.uniform(*std["sn"]), 2),
            "alloy_pb": round(random.uniform(*std["pb"]), 2),
            "alloy_zn": round(random.uniform(*std["zn"]), 2),
            "alloy_other": round(random.uniform(*std["other"]), 2),
        }

    def _generate_temperature(
        self, hour_in_cycle: int, inject_error: bool = False
    ) -> Tuple[float, float]:
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

    def _generate_wall_thickness(
        self, bell: Dict, inject_error: bool = False
    ) -> Tuple[float, float]:
        diameter_m = bell.get("diameter_m", 1.0)
        base_thickness_mm = diameter_m * 1000 * 0.04

        if inject_error:
            deviation_pct = random.choice(
                [random.uniform(-30, -15), random.uniform(15, 30)]
            )
        else:
            deviation_pct = random.uniform(-8, 8)

        thickness = base_thickness_mm * (1 + deviation_pct / 100)
        return round(thickness, 2), round(deviation_pct, 2)

    def _generate_acoustic(
        self, bell: Dict, inject_error: bool = False
    ) -> Tuple[float, float, float, List[float]]:
        expected_freq = bell.get("expected_freq_hz", 261.63)

        if inject_error:
            freq_error_cents = random.choice(
                [random.uniform(-200, -80), random.uniform(80, 200)]
            )
            freq = expected_freq * (2 ** (freq_error_cents / 1200))
        else:
            freq_error_cents = random.uniform(-40, 40)
            freq = expected_freq * (2 ** (freq_error_cents / 1200))

        harmonics = [freq * (2 + i * 0.5 + random.uniform(-0.05, 0.05)) for i in range(6)]
        amplitude = random.uniform(0.6, 1.0)
        decay = random.uniform(0.3, 0.8)

        if inject_error:
            amplitude = random.uniform(0.2, 0.4)
            decay = random.uniform(1.2, 2.0)

        return (
            round(freq, 4),
            round(amplitude, 4),
            round(decay, 4),
            [round(h, 4) for h in harmonics],
        )

    def generate_sensor_reading(
        self,
        bell: Dict,
        hour_in_cycle: int,
        inject_alloy_error: bool = False,
        inject_temp_error: bool = False,
        inject_thickness_error: bool = False,
        inject_acoustic_error: bool = False,
    ) -> Dict:
        alloy_type = self._get_alloy_type(bell)
        alloy = self._generate_alloy(alloy_type, inject_alloy_error)
        temp, temp_grad = self._generate_temperature(hour_in_cycle, inject_temp_error)
        thickness, deviation = self._generate_wall_thickness(bell, inject_thickness_error)
        freq, amp, decay, harmonics = self._generate_acoustic(bell, inject_acoustic_error)

        return {
            "bell_id": bell["bell_id"],
            "temp_celsius": temp,
            "temp_gradient": temp_grad,
            "wall_thickness_mm": thickness,
            "thickness_deviation": deviation,
            **alloy,
            "acoustic_freq_hz": freq,
            "acoustic_amplitude": amp,
            "acoustic_decay": decay,
            "acoustic_harmonics": harmonics,
        }

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

        for stage_name, duration_sec, temp_start, temp_end, fill_end in CASTING_STAGES:
            if stage_name == "finished":
                process = {
                    "bell_id": bell_id,
                    "stage": stage_name,
                    "progress": 1.0,
                    "current_temp": temp_end,
                    "mold_fill_level": fill_end,
                }
                self.session.post(f"{self.backend_url}/casting-process", json=process)
                print(f"  [{stage_name:12s}] 完成 100%")
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
                        f"  [{stage_name:12s}] {progress*100:5.1f}% | "
                        f"温度: {current_temp:7.1f}°C | 填充: {fill_level*100:5.1f}%"
                    )

                if accelerated:
                    time.sleep(0.05)
                else:
                    time.sleep(max(0.1, step_duration / 10))

        print(f"=== 铸造完成: {bell['bell_name']} ===")

    def run_casting_simulation(
        self,
        bell: Dict,
        initial_temp: Optional[float] = None,
        sim_type: str = "solidification",
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
        method: str = "FEM",
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
                freqs = sim["natural_frequencies"][:4]
                status = "合格" if sim["pitch_ok"] else "偏差"
                print(
                    f"  [声学仿真] {bell['bell_name']} | "
                    f"音准: {status} ({sim['pitch_deviation_cents']:+.1f}音分) | "
                    f"基频: {freqs[0]:.2f}Hz | "
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
                    status_flags.append("成分异常")
                if inject_temp:
                    status_flags.append("温度异常")
                if inject_thickness:
                    status_flags.append("壁厚异常")
                if inject_acoustic:
                    status_flags.append("音准异常")
                flag_str = f" [注入: {','.join(status_flags)}]" if status_flags else ""
                alert_str = f" -> {alerts}个告警" if alerts > 0 else ""

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


def main():
    parser = argparse.ArgumentParser(description="古代铸钟工艺模拟器")
    parser.add_argument(
        "--backend", default=BACKEND_URL, help=f"后端URL (默认: {BACKEND_URL})"
    )
    parser.add_argument("--hours", type=int, default=72, help="模拟总小时数")
    parser.add_argument(
        "--error-rate", type=float, default=0.05, help="异常数据注入率 (0-1)"
    )
    parser.add_argument(
        "--sleep", type=float, default=1.0, help="每小时间隔秒数 (加速模拟)"
    )
    parser.add_argument(
        "--casting", action="store_true", help="对所有钟执行铸造过程动画模拟"
    )
    parser.add_argument("--sim-casting", action="store_true", help="立即运行铸造仿真")
    parser.add_argument("--sim-acoustic", action="store_true", help="立即运行声学仿真")
    parser.add_argument(
        "--accelerated", action="store_true", help="加速铸造过程模拟"
    )
    parser.add_argument(
        "--once", action="store_true", help="只运行一轮传感器上报后退出"
    )
    args = parser.parse_args()

    sim = BellSimulator(args.backend)
    if not sim.bells:
        print("[错误] 没有可用钟体，请先初始化ClickHouse数据库并插入钟体信息")
        sys.exit(1)

    if args.casting:
        for bell in sim.bells:
            sim.simulate_casting_process(bell, accelerated=args.accelerated)

    if args.sim_casting:
        print("\n=== 立即铸造仿真 ===")
        for bell in sim.bells:
            sim.run_casting_simulation(bell)

    if args.sim_acoustic:
        print("\n=== 立即声学仿真 ===")
        for bell in sim.bells:
            sim.run_acoustic_simulation(bell)

    if args.once:
        hours = 1
    else:
        hours = args.hours

    if not (args.casting and not args.once and not (args.sim_casting or args.sim_acoustic)):
        sim.run_hourly_simulation(
            total_hours=hours,
            error_rate=args.error_rate,
            sleep_sec=args.sleep,
        )

    print("\n模拟完成。")


if __name__ == "__main__":
    main()
