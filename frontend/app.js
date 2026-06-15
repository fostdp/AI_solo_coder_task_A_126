import { Bell3D } from './bell_3d.js';
import { AcousticPanel } from './acoustic_panel.js';

const API_BASE = 'http://localhost:8080';

const state = {
    bells: [],
    selectedBell: null,
    viewMode: '3d',
    bell3d: null,
    acousticPanel: null,
    currentCastingSim: null,
    currentAcousticSim: null,
};

const bellNameMap = {};
const bellById = {};

function $(id) { return document.getElementById(id); }

async function api(path, method = 'GET', body = null) {
    const opts = {
        method,
        headers: { 'Content-Type': 'application/json' },
    };
    if (body) opts.body = JSON.stringify(body);
    try {
        const resp = await fetch(API_BASE + path, opts);
        const data = await resp.json();
        return data;
    } catch (e) {
        console.warn('API error:', path, e);
        return { success: false, error: String(e) };
    }
}

function formatTime(iso) {
    if (!iso) return '-';
    const d = new Date(iso);
    return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
}

/* ========================= 钟列表渲染 ========================= */
async function loadBells() {
    const res = await api('/bells');
    if (res.success && res.data) {
        state.bells = res.data;
        res.data.forEach(b => {
            bellNameMap[b.bell_id] = b.bell_name;
            bellById[b.bell_id] = b;
        });
        renderBellList();
        if (state.bells.length > 0) {
            selectBell(state.bells[0].bell_id);
        }
    }
}

function renderBellList() {
    const el = $('bell-list');
    el.innerHTML = '';
    state.bells.forEach(b => {
        const card = document.createElement('div');
        card.className = 'bell-card' + (state.selectedBell?.bell_id === b.bell_id ? ' active' : '');
        card.innerHTML = `
            <div class="name">${b.bell_name}</div>
            <div class="meta"><span>${b.dynasty}</span><span>${b.weight_kg}kg</span></div>
            <span class="pitch">${b.expected_pitch} · ${b.expected_freq_hz.toFixed(2)}Hz</span>
        `;
        card.onclick = () => selectBell(b.bell_id);
        el.appendChild(card);
    });
}

function selectBell(bellId) {
    const bell = bellById[bellId];
    if (!bell) return;
    state.selectedBell = bell;
    renderBellList();
    updateBellInfo(bell);
    state.bell3d.buildBellMesh(bell);
    state.acousticPanel.setBell(bell);
    loadLatestSensorData(bellId);
    loadSimulations(bellId);
}

function updateBellInfo(b) {
    $('bi-name').textContent = b.bell_name;
    $('bi-dynasty').textContent = `${b.dynasty} · ${b.bell_type}`;
    $('bi-height').textContent = `${b.height_m.toFixed(2)} m`;
    $('bi-diameter').textContent = `${b.diameter_m.toFixed(2)} m`;
    $('bi-weight').textContent = `${b.weight_kg.toFixed(1)} kg`;
    $('bi-pitch').textContent = `${b.expected_pitch} · ${b.expected_freq_hz.toFixed(2)} Hz`;
}

/* ========================= 数据加载 & UI ========================= */
async function loadLatestSensorData(bellId) {
    const res = await api(`/sensors/bell/${bellId}?limit=1`);
    if (res.success && res.data && res.data[0]) {
        updateSensorDisplay(res.data[0]);
    }
}

function updateSensorDisplay(r) {
    $('bi-temp').textContent = `${r.temp_celsius.toFixed(1)} °C`;
    $('bi-thickness').textContent = `${r.wall_thickness_mm.toFixed(2)} mm`;
    $('bi-freq').textContent = `${r.acoustic_freq_hz.toFixed(2)} Hz`;
    $('m-temp').textContent = `${Math.round(r.temp_celsius)}°C`;
    $('m-freq').textContent = `${Math.round(r.acoustic_freq_hz)}Hz`;
    $('m-cu').textContent = `${r.alloy_cu.toFixed(1)}%`;
    $('m-sn').textContent = `${r.alloy_sn.toFixed(1)}%`;
    state.bell3d.setVibrationAmp(r.acoustic_amplitude);
    state.acousticPanel.renderFreqBars(r.acoustic_harmonics || []);
}

async function loadSimulations(bellId) {
    const [cast, acou] = await Promise.all([
        api(`/sim/casting/bell/${bellId}?limit=1`),
        api(`/sim/acoustic/bell/${bellId}?limit=1`),
    ]);
    if (cast.success && cast.data?.[0]) {
        state.currentCastingSim = cast.data[0];
        if (state.viewMode === 'defect') state.bell3d.visualizeDefects(cast.data[0]);
    }
    if (acou.success && acou.data?.[0]) {
        state.currentAcousticSim = acou.data[0];
        state.acousticPanel.setAcousticSim(acou.data[0]);
        const freqs = acou.data[0].natural_frequencies;
        if (freqs?.length) state.acousticPanel.renderFreqBars(freqs);
    }
}

async function loadAlerts() {
    const res = await api('/alerts');
    if (res.success && res.data) {
        renderAlerts(res.data);
    }
}

function renderAlerts(alerts) {
    $('alert-count').textContent = `(${alerts.length})`;
    const el = $('alert-list');
    el.innerHTML = '';
    if (!alerts.length) {
        el.innerHTML = '<div style="text-align:center;color:#4a5568;padding:20px;font-size:11px;">暂无告警</div>';
        return;
    }
    alerts.slice(0, 30).forEach(a => {
        const typeNames = {
            defect: '铸造缺陷', pitch: '音准偏差', temp: '温度异常', alloy: '成分异常',
        };
        const sev = a.severity || 'warning';
        const bell = bellNameMap[a.bell_id] || '未知';
        const item = document.createElement('div');
        item.className = `alert-item ${sev}`;
        item.innerHTML = `
            <div class="header">
                <span class="bell">${bell}</span>
                <span class="sev ${sev}">${typeNames[a.alert_type] || a.alert_type} · ${sev}</span>
            </div>
            <div class="msg">${a.message}</div>
            <div class="time">${formatTime(a.timestamp)}</div>
        `;
        item.onclick = async () => {
            if (confirm('确认处理此告警？')) {
                await api(`/alerts/${a.alert_id}/resolve`, 'POST');
                loadAlerts();
            }
        };
        el.appendChild(item);
    });
}

/* ========================= 事件绑定 ========================= */
function bindEvents() {
    $('view-tabs').addEventListener('click', (e) => {
        const tab = e.target.closest('.view-tab');
        if (!tab) return;
        document.querySelectorAll('.view-tab').forEach(t => t.classList.remove('active'));
        tab.classList.add('active');
        state.viewMode = tab.dataset.view;

        state.bell3d.setViewMode(state.viewMode);
        state.acousticPanel.setViewMode(state.viewMode);
    });

    $('btn-sim-casting').onclick = async () => {
        if (!state.selectedBell) return;
        const res = await api('/sim/casting', 'POST', {
            bell_id: state.selectedBell.bell_id,
            sim_type: 'solidification',
            initial_temp: 1180,
            grid_size: 20,
        });
        if (res.success) {
            state.currentCastingSim = res.data;
            if (state.viewMode === 'defect') state.bell3d.visualizeDefects(res.data);
            alert(`铸造仿真完成！\n风险等级: ${res.data.prediction_risk}\n最大缩孔率: ${(res.data.max_shrinkage*100).toFixed(2)}%\n缺陷数: ${res.data.defect_count}`);
        }
    };

    $('btn-sim-acoustic').onclick = async () => {
        if (!state.selectedBell) return;
        const res = await api('/sim/acoustic', 'POST', {
            bell_id: state.selectedBell.bell_id,
            method: 'FEM',
        });
        if (res.success) {
            state.currentAcousticSim = res.data;
            state.acousticPanel.setAcousticSim(res.data);
            const freqs = res.data.natural_frequencies;
            if (freqs?.length) state.acousticPanel.renderFreqBars(freqs);
            alert(`声学仿真完成！\n音准: ${res.data.pitch_ok ? '合格' : '偏差'} (${res.data.pitch_deviation_cents.toFixed(1)}音分)\n基频: ${freqs?.[0]?.toFixed(2) || '-'}Hz\n声功率: ${res.data.sound_power.toFixed(4)}W`);
        }
    };

    $('btn-strike').onclick = () => {
        state.bell3d.strike();
        playBellSound();
    };

    $('btn-rotate').onclick = () => {
        const rotating = state.bell3d.toggleAutoRotate();
        $('btn-rotate').textContent = rotating ? '⏸ 停止旋转' : '⟳ 自动旋转';
    };

    $('btn-sim-all').onclick = async () => {
        if (!state.selectedBell) return;
        state.viewMode = 'casting';
        document.querySelectorAll('.view-tab').forEach(t => {
            t.classList.toggle('active', t.dataset.view === 'casting');
        });
        state.bell3d.setViewMode(state.viewMode);
        state.acousticPanel.setViewMode(state.viewMode);
        state.bell3d.runCastingAnimation();
        setTimeout(() => $('btn-sim-casting').click(), 13000);
        setTimeout(() => $('btn-sim-acoustic').click(), 15000);
    };
}

/* ========================= 钟声音效 ========================= */
function playBellSound() {
    try {
        const ctx = new (window.AudioContext || window.webkitAudioContext)();
        const master = ctx.createGain();
        master.connect(ctx.destination);
        master.gain.value = 0.3;

        const fundFreq = state.selectedBell?.expected_freq_hz || 261.63;
        const partials = [
            { f: 1.0, g: 0.8, d: 3.0 },
            { f: 2.0, g: 0.4, d: 2.5 },
            { f: 2.76, g: 0.35, d: 4.0 },
            { f: 4.2, g: 0.25, d: 3.5 },
            { f: 5.4, g: 0.15, d: 2.0 },
        ];
        const now = ctx.currentTime;
        partials.forEach(p => {
            const osc = ctx.createOscillator();
            osc.type = 'sine';
            osc.frequency.value = fundFreq * p.f;

            const gain = ctx.createGain();
            gain.gain.setValueAtTime(0, now);
            gain.gain.linearRampToValueAtTime(p.g, now + 0.005);
            gain.gain.exponentialRampToValueAtTime(0.001, now + p.d);

            osc.connect(gain);
            gain.connect(master);
            osc.start(now);
            osc.stop(now + p.d);
        });
    } catch (e) {
        console.warn('Audio not available', e);
    }
}

/* ========================= 初始化 ========================= */
function initClock() {
    const update = () => {
        $('clock').textContent = new Date().toLocaleString('zh-CN');
    };
    update();
    setInterval(update, 1000);
}

async function init() {
    state.bell3d = new Bell3D('main-canvas');
    state.acousticPanel = new AcousticPanel('sound-field-canvas');

    state.bell3d.init();
    state.acousticPanel.init();

    bindEvents();
    initClock();
    await loadBells();
    loadAlerts();
    setInterval(loadAlerts, 10000);
    setInterval(() => {
        if (state.selectedBell) loadLatestSensorData(state.selectedBell.bell_id);
    }, 5000);

    console.log('🚀 古代铸钟仿真系统前端已加载 (微服务架构)');
}

init();
