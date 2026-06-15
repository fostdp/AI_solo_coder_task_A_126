import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

const API_BASE = 'http://localhost:8080';

const state = {
    bells: [],
    selectedBell: null,
    viewMode: '3d',
    autoRotate: false,
    scene: null,
    camera: null,
    renderer: null,
    controls: null,
    bellGroup: null,
    particles: null,
    moltenMetal: null,
    defectMeshes: [],
    currentCastingSim: null,
    currentAcousticSim: null,
    castingStage: null,
    clock: new THREE.Clock(),
    vibrationAmp: 0,
    strikeIntensity: 0,
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
    buildBellMesh(bell);
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

/* ========================= 3D场景初始化 ========================= */
function initThree() {
    const canvas = $('main-canvas');
    state.scene = new THREE.Scene();
    state.scene.background = new THREE.Color(0x05080f);
    state.scene.fog = new THREE.Fog(0x05080f, 15, 40);

    const w = canvas.clientWidth, h = canvas.clientHeight;
    state.camera = new THREE.PerspectiveCamera(45, w / h, 0.1, 1000);
    state.camera.position.set(3, 2.5, 5);

    state.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    state.renderer.setPixelRatio(window.devicePixelRatio);
    state.renderer.setSize(w, h, false);
    state.renderer.shadowMap.enabled = true;
    state.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    state.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    state.renderer.toneMappingExposure = 1.1;

    state.controls = new OrbitControls(state.camera, canvas);
    state.controls.enableDamping = true;
    state.controls.dampingFactor = 0.08;
    state.controls.minDistance = 2;
    state.controls.maxDistance = 20;
    state.controls.target.set(0, 0, 0);

    const ambient = new THREE.AmbientLight(0x404060, 0.6);
    state.scene.add(ambient);

    const keyLight = new THREE.DirectionalLight(0xffe5b3, 1.0);
    keyLight.position.set(5, 8, 5);
    keyLight.castShadow = true;
    keyLight.shadow.mapSize.set(1024, 1024);
    state.scene.add(keyLight);

    const rimLight = new THREE.DirectionalLight(0x8888ff, 0.4);
    rimLight.position.set(-5, 3, -5);
    state.scene.add(rimLight);

    const fillLight = new THREE.PointLight(0xe8c468, 0.8, 20);
    fillLight.position.set(0, 2, 0);
    state.scene.add(fillLight);

    const floorGeo = new THREE.CircleGeometry(12, 64);
    const floorMat = new THREE.MeshStandardMaterial({
        color: 0x1a1e2e,
        metalness: 0.3,
        roughness: 0.8,
    });
    const floor = new THREE.Mesh(floorGeo, floorMat);
    floor.rotation.x = -Math.PI / 2;
    floor.position.y = -1.8;
    floor.receiveShadow = true;
    state.scene.add(floor);

    addFloorPattern();
    addParticles();

    window.addEventListener('resize', onResize);
    animate();
}

function addFloorPattern() {
    const group = new THREE.Group();
    const ringMat = new THREE.LineBasicMaterial({ color: 0x2a3552, transparent: true, opacity: 0.4 });
    for (let r = 2; r <= 10; r += 2) {
        const points = [];
        for (let i = 0; i <= 64; i++) {
            const a = (i / 64) * Math.PI * 2;
            points.push(new THREE.Vector3(Math.cos(a) * r, -1.79, Math.sin(a) * r));
        }
        const geo = new THREE.BufferGeometry().setFromPoints(points);
        group.add(new THREE.Line(geo, ringMat));
    }
    state.scene.add(group);
}

function addParticles() {
    const count = 300;
    const geo = new THREE.BufferGeometry();
    const positions = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
        positions[i*3] = (Math.random() - 0.5) * 20;
        positions[i*3+1] = Math.random() * 10 - 2;
        positions[i*3+2] = (Math.random() - 0.5) * 20;
    }
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    const mat = new THREE.PointsMaterial({
        color: 0xe8c468,
        size: 0.03,
        transparent: true,
        opacity: 0.5,
    });
    state.particles = new THREE.Points(geo, mat);
    state.scene.add(state.particles);
}

function onResize() {
    const canvas = $('main-canvas');
    const w = canvas.clientWidth, h = canvas.clientHeight;
    state.camera.aspect = w / h;
    state.camera.updateProjectionMatrix();
    state.renderer.setSize(w, h, false);
    drawSoundField();
}

/* ========================= 铸钟模型构建 ========================= */
function buildBellMesh(bell) {
    if (state.bellGroup) state.scene.remove(state.bellGroup);
    state.defectMeshes.forEach(m => state.scene.remove(m));
    state.defectMeshes = [];
    if (state.moltenMetal) { state.scene.remove(state.moltenMetal); state.moltenMetal = null; }

    state.bellGroup = new THREE.Group();
    const scale = Math.max(0.8, Math.min(3, bell.height_m));
    state.bellGroup.scale.setScalar(scale);

    const height = 2.0;
    const topR = 0.55;
    const midR = 0.75;
    const botR = 0.95;
    const thickness = 0.08;

    const bellMat = new THREE.MeshStandardMaterial({
        color: 0xa0751a,
        metalness: 0.88,
        roughness: 0.32,
    });

    const outerPoints = [];
    const N = 60;
    for (let i = 0; i <= N; i++) {
        const t = i / N;
        let r;
        if (t < 0.15) {
            r = topR + (midR - topR) * (t / 0.15) * 0.6;
        } else if (t < 0.7) {
            const lt = (t - 0.15) / 0.55;
            r = midR * (1 + 0.08 * Math.sin(lt * Math.PI));
        } else {
            const lt = (t - 0.7) / 0.3;
            r = midR + (botR - midR) * (1 - Math.cos(lt * Math.PI / 2));
        }
        const y = height / 2 - t * height;
        outerPoints.push(new THREE.Vector2(r, y));
    }
    for (let i = N; i >= 0; i--) {
        const t = i / N;
        let r;
        if (t < 0.15) {
            r = topR - thickness * 0.8 + (midR - topR) * (t / 0.15) * 0.6;
        } else if (t < 0.7) {
            const lt = (t - 0.15) / 0.55;
            r = (midR - thickness) * (1 + 0.08 * Math.sin(lt * Math.PI));
        } else {
            const lt = (t - 0.7) / 0.3;
            r = (midR - thickness) + (botR - midR - thickness * 1.2) * (1 - Math.cos(lt * Math.PI / 2));
        }
        const y = height / 2 - t * height;
        outerPoints.push(new THREE.Vector2(Math.max(0.05, r), y));
    }

    const outerGeo = new THREE.LatheGeometry(outerPoints, 64);
    const bellMesh = new THREE.Mesh(outerGeo, bellMat);
    bellMesh.castShadow = true;
    bellMesh.receiveShadow = true;
    state.bellGroup.add(bellMesh);

    const knobGeo = new THREE.SphereGeometry(0.12, 24, 24);
    const knobMat = new THREE.MeshStandardMaterial({
        color: 0xd4af37,
        metalness: 0.95,
        roughness: 0.2,
    });
    const knob = new THREE.Mesh(knobGeo, knobMat);
    knob.position.y = height / 2 + 0.1;
    knob.castShadow = true;
    state.bellGroup.add(knob);

    const crownGeo = new THREE.TorusGeometry(0.18, 0.025, 12, 32);
    const crown = new THREE.Mesh(crownGeo, knobMat);
    crown.position.y = height / 2 + 0.22;
    crown.rotation.x = Math.PI / 2;
    crown.castShadow = true;
    state.bellGroup.add(crown);

    const rimGeo = new THREE.TorusGeometry(botR - 0.01, 0.035, 16, 64);
    const rimMat = new THREE.MeshStandardMaterial({
        color: 0x8b6914,
        metalness: 0.85,
        roughness: 0.4,
    });
    const rim = new THREE.Mesh(rimGeo, rimMat);
    rim.position.y = -height / 2 + 0.02;
    rim.rotation.x = Math.PI / 2;
    rim.castShadow = true;
    state.bellGroup.add(rim);

    addDecorativeRings(state.bellGroup, height, midR);
    addInscriptions(state.bellGroup, height, midR);

    state.bellGroup.position.y = 0;
    state.scene.add(state.bellGroup);

    const fitDist = 3 / scale + 2;
    state.camera.position.set(fitDist, fitDist * 0.6, fitDist);
    state.controls.target.set(0, 0, 0);
    state.controls.update();
}

function addDecorativeRings(group, height, midR) {
    const ringMat = new THREE.MeshStandardMaterial({
        color: 0x6b4c0a,
        metalness: 0.7,
        roughness: 0.5,
    });
    const positions = [0.3, 0.5, 0.65];
    positions.forEach((t, idx) => {
        const y = height / 2 - t * height;
        const r = midR * (1 + 0.05 * Math.sin(((t - 0.15) / 0.55) * Math.PI)) - 0.005;
        const torus = new THREE.Mesh(
            new THREE.TorusGeometry(r, 0.012, 10, 64),
            ringMat
        );
        torus.position.y = y;
        torus.rotation.x = Math.PI / 2;
        group.add(torus);

        for (let i = 0; i < 8; i++) {
            const a = (i / 8) * Math.PI * 2;
            const boss = new THREE.Mesh(
                new THREE.SphereGeometry(0.022, 12, 12),
                ringMat
            );
            boss.position.set(
                Math.cos(a) * r,
                y,
                Math.sin(a) * r
            );
            group.add(boss);
        }
    });
}

function addInscriptions(group, height, midR) {
    const mat = new THREE.MeshStandardMaterial({
        color: 0x2a1a05,
        metalness: 0.5,
        roughness: 0.7,
    });
    for (let row = 0; row < 3; row++) {
        const t = 0.25 + row * 0.15;
        const y = height / 2 - t * height;
        const r = midR * (1 + 0.05 * Math.sin(((t - 0.15) / 0.55) * Math.PI)) - 0.002;
        for (let i = 0; i < 12; i++) {
            const a = (i / 12) * Math.PI * 2;
            const char = new THREE.Mesh(
                new THREE.BoxGeometry(0.025, 0.035, 0.004),
                mat
            );
            char.position.set(
                Math.cos(a) * r,
                y,
                Math.sin(a) * r
            );
            char.lookAt(0, y, 0);
            char.translateZ(0.002);
            group.add(char);
        }
    }
}

/* ========================= 铸造动画 ========================= */
function runCastingAnimation() {
    if (!state.selectedBell) return;
    $('casting-stage-bar').style.display = 'block';

    const stages = [
        { name: '制模阶段', key: 'molding', dur: 1500, func: animateMolding },
        { name: '熔炼阶段', key: 'melting', dur: 2000, func: animateMelting },
        { name: '浇注阶段', key: 'pouring', dur: 3000, func: animatePouring },
        { name: '冷却阶段', key: 'cooling', dur: 2500, func: animateCooling },
        { name: '凝固阶段', key: 'solidifying', dur: 2500, func: animateSolidifying },
        { name: '铸造完成', key: 'finished', dur: 1000, func: animateFinished },
    ];

    const totalDur = stages.reduce((s, x) => s + x.dur, 0);
    let elapsed = 0;

    stages.forEach((stage, idx) => {
        const stageStart = elapsed;
        setTimeout(() => {
            $('stage-name').textContent = stage.name;
            const overallPct = Math.round((stageStart / totalDur) * 100);
            $('progress-fill').style.width = overallPct + '%';
            $('stage-progress').textContent = overallPct + '%';
            state.castingStage = stage.key;
            stage.func(stage.dur);
        }, stageStart);
        elapsed += stage.dur;
    });

    setTimeout(() => {
        $('progress-fill').style.width = '100%';
        $('stage-progress').textContent = '100%';
    }, totalDur);
}

function animateMolding(dur) {
    if (state.bellGroup) state.bellGroup.visible = false;
    const moldMat = new THREE.MeshStandardMaterial({
        color: 0x8b4513,
        metalness: 0.1,
        roughness: 0.9,
        transparent: true,
        opacity: 0.7,
    });
    const mold = new THREE.Mesh(
        new THREE.CylinderGeometry(1.2, 1.2, 2.5, 32, 1, true),
        moldMat
    );
    mold.position.y = -0.2;
    state.scene.add(mold);
    animate()._tempMold = mold;

    let t = 0;
    const step = () => {
        t += 16;
        const p = Math.min(1, t / dur);
        mold.material.opacity = 0.4 + p * 0.4;
        if (p < 1) requestAnimationFrame(step);
    };
    step();
}

function animateMelting(dur) {
    const metalGroup = new THREE.Group();
    state.scene.add(metalGroup);
    animate()._tempMetal = metalGroup;

    const crucibleGeo = new THREE.CylinderGeometry(0.6, 0.4, 1.2, 24);
    const crucibleMat = new THREE.MeshStandardMaterial({
        color: 0x2a1810,
        roughness: 0.95,
        metalness: 0,
    });
    const crucible = new THREE.Mesh(crucibleGeo, crucibleMat);
    crucible.position.set(0, 2.5, 2);
    metalGroup.add(crucible);

    const meltGeo = new THREE.CylinderGeometry(0.55, 0.38, 0.9, 24);
    const meltMat = new THREE.MeshStandardMaterial({
        color: 0xff5500,
        emissive: 0xff2200,
        emissiveIntensity: 2,
        roughness: 0.2,
        metalness: 0.8,
    });
    const melt = new THREE.Mesh(meltGeo, meltMat);
    melt.position.set(0, 2.5, 2);
    metalGroup.add(melt);

    const light = new THREE.PointLight(0xff6600, 2, 10);
    light.position.set(0, 2.5, 2);
    metalGroup.add(light);

    let t = 0;
    const step = () => {
        t += 16;
        const p = Math.min(1, t / dur);
        meltMat.emissiveIntensity = 1 + p * 2;
        meltMat.color.setHSL(0.05, 1, 0.5 + p * 0.1);
        melt.material.needsUpdate = true;
        if (p < 1) requestAnimationFrame(step);
    };
    step();
}

function animatePouring(dur) {
    const tempMetal = animate()._tempMetal;
    state.moltenMetal = tempMetal;

    const streamGeo = new THREE.CylinderGeometry(0.05, 0.08, 3, 16);
    const streamMat = new THREE.MeshStandardMaterial({
        color: 0xff3300,
        emissive: 0xff4400,
        emissiveIntensity: 3,
    });
    const stream = new THREE.Mesh(streamGeo, streamMat);
    stream.position.set(0, 0.5, 1.2);
    stream.rotation.z = Math.PI / 8;
    state.scene.add(stream);

    const fillMat = new THREE.MeshStandardMaterial({
        color: 0xff4400,
        emissive: 0xff2200,
        emissiveIntensity: 2.5,
    });
    const fill = new THREE.Mesh(
        new THREE.CylinderGeometry(0.5, 0.5, 0.01, 32),
        fillMat
    );
    fill.position.y = -1.2;
    state.scene.add(fill);

    let t = 0;
    const step = () => {
        t += 16;
        const p = Math.min(1, t / dur);
        fill.scale.y = 1 + p * 200;
        fill.position.y = -1.2 + p * 1.0;
        fillMat.emissiveIntensity = 2.5 * (1 - p * 0.3);
        stream.scale.y = 1 - p * 0.9;
        if (p < 1) requestAnimationFrame(step);
        else {
            state.scene.remove(stream);
            state.scene.remove(fill);
        }
    };
    step();
}

function animateCooling(dur) {
    const mold = animate()._tempMold;
    let t = 0;
    const step = () => {
        t += 16;
        const p = Math.min(1, t / dur);
        if (mold) mold.material.opacity = 0.8 * (1 - p);
        if (p < 1) requestAnimationFrame(step);
        else {
            if (mold) state.scene.remove(mold);
            if (state.bellGroup) {
                state.bellGroup.visible = true;
                state.bellGroup.children.forEach(c => {
                    if (c.material && c.material.color) {
                        const orig = c.material.color.clone();
                        const hot = new THREE.Color(0xff4400);
                        c.material.color.copy(hot).lerp(orig, p);
                    }
                });
            }
        }
    };
    step();
}

function animateSolidifying(dur) {
    let t = 0;
    const step = () => {
        t += 16;
        const p = Math.min(1, t / dur);
        if (state.bellGroup) {
            state.bellGroup.children.forEach(c => {
                if (c.material) {
                    if (c.material.emissive) {
                        c.material.emissiveIntensity = Math.max(0, 1.5 * (1 - p));
                    }
                }
            });
        }
        if (p < 1) requestAnimationFrame(step);
    };
    step();
}

function animateFinished(dur) {
    if (state.moltenMetal) {
        state.scene.remove(state.moltenMetal);
        state.moltenMetal = null;
    }
    if (state.bellGroup) {
        state.bellGroup.children.forEach(c => {
            if (c.material && c.material.emissive) c.material.emissiveIntensity = 0;
        });
    }
}

/* ========================= 铸造仿真缺陷可视化 ========================= */
function visualizeDefects(sim) {
    state.defectMeshes.forEach(m => state.scene.remove(m));
    state.defectMeshes = [];
    if (!sim || !sim.defect_locations) return;

    sim.defect_locations.forEach((loc, i) => {
        const [x, y, z, sev] = loc;
        const scale = sev * 3 + 0.3;
        const hue = sev > 0.05 ? 0 : sev > 0.03 ? 0.08 : 0.15;
        const geo = new THREE.SphereGeometry(scale * 0.08, 12, 12);
        const mat = new THREE.MeshStandardMaterial({
            color: new THREE.Color().setHSL(hue, 1, 0.5),
            transparent: true,
            opacity: 0.75,
            emissive: new THREE.Color().setHSL(hue, 1, 0.3),
            emissiveIntensity: 1,
        });
        const mesh = new THREE.Mesh(geo, mat);
        const bell = state.selectedBell;
        const s = Math.max(0.8, Math.min(3, bell.height_m));
        mesh.position.set(
            (x - 0.5) * 2 * s,
            (y - 0.5) * 2 * s,
            (z - 0.5) * 2 * s
        );
        state.scene.add(mesh);
        state.defectMeshes.push(mesh);
    });
}

/* ========================= 声场云图 ========================= */
function drawSoundField() {
    const canvas = $('sound-field-canvas');
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    const ctx = canvas.getContext('2d');
    ctx.scale(dpr, dpr);

    ctx.clearRect(0, 0, w, h);

    const freq = state.currentAcousticSim
        ? state.currentAcousticSim.natural_frequencies?.[0] || 261.63
        : state.selectedBell?.expected_freq_hz || 261.63;

    const soundSpeed = 343;
    const lambda = soundSpeed / freq;
    const k = 2 * Math.PI / lambda;

    const cx = w / 2;
    const cy = h * 0.35;

    const imgData = ctx.createImageData(w, h);
    const data = imgData.data;
    const srcStrength = 1 / Math.max(20, state.selectedBell?.weight_kg || 50);

    for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
            const dx = (x - cx) / w * 15;
            const dy = (y - cy) / h * 15;
            const r = Math.sqrt(dx * dx + dy * dy);
            const theta = Math.atan2(dy, dx);

            let pressure;
            if (r < 0.2) {
                pressure = 1;
            } else {
                const directivity = 1 + 0.6 * Math.pow(Math.cos(theta - Math.PI / 2), 2);
                const wave = Math.sin(k * r * 0.3 - performance.now() * 0.002) / Math.sqrt(r + 0.1);
                pressure = Math.abs(wave * directivity * srcStrength * 500);
            }

            pressure = Math.min(1, pressure / 1.5);
            const idx = (y * w + x) * 4;

            if (pressure < 0.01) {
                data[idx] = 5; data[idx+1] = 8; data[idx+2] = 15; data[idx+3] = 255;
            } else {
                const hue = (1 - pressure) * 0.65;
                const [rr, gg, bb] = hsl2rgb(hue, 0.85, 0.4 + pressure * 0.3);
                data[idx] = rr; data[idx+1] = gg; data[idx+2] = bb; data[idx+3] = 255;
            }
        }
    }
    ctx.putImageData(imgData, 0, 0);

    const grad = ctx.createRadialGradient(cx, cy, 5, cx, cy, 30);
    grad.addColorStop(0, 'rgba(255,220,120,0.9)');
    grad.addColorStop(0.5, 'rgba(232,196,104,0.5)');
    grad.addColorStop(1, 'rgba(232,196,104,0)');
    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.arc(cx, cy, 30, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = '#e8c468';
    ctx.font = 'bold 14px "Microsoft YaHei"';
    ctx.fillText(`声压级云图 · ${freq.toFixed(1)}Hz · λ=${lambda.toFixed(2)}m`, 20, 30);
    ctx.font = '11px "Microsoft YaHei"';
    ctx.fillStyle = '#8892b0';
    ctx.fillText('图例: 蓝(低) → 青 → 绿 → 黄 → 红(高)', 20, 50);

    drawColorBar(ctx, w - 40, 60, 20, h - 120);
}

function drawColorBar(ctx, x, y, w, h) {
    for (let i = 0; i < h; i++) {
        const t = 1 - i / h;
        const hue = (1 - t) * 0.65;
        const [r, g, b] = hsl2rgb(hue, 0.85, 0.4 + t * 0.3);
        ctx.fillStyle = `rgb(${r},${g},${b})`;
        ctx.fillRect(x, y + i, w, 1);
    }
    ctx.strokeStyle = '#2a3552';
    ctx.lineWidth = 1;
    ctx.strokeRect(x, y, w, h);
    ctx.fillStyle = '#8892b0';
    ctx.font = '10px monospace';
    ctx.textAlign = 'left';
    ctx.fillText('高', x + w + 6, y + 8);
    ctx.fillText('低', x + w + 6, y + h);
}

function hsl2rgb(h, s, l) {
    let r, g, b;
    if (s === 0) {
        r = g = b = l;
    } else {
        const hue2rgb = (p, q, t) => {
            if (t < 0) t += 1;
            if (t > 1) t -= 1;
            if (t < 1/6) return p + (q - p) * 6 * t;
            if (t < 1/2) return q;
            if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
            return p;
        };
        const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
        const p = 2 * l - q;
        r = hue2rgb(p, q, h + 1/3);
        g = hue2rgb(p, q, h);
        b = hue2rgb(p, q, h - 1/3);
    }
    return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255)];
}

/* ========================= 动画主循环 ========================= */
function animate() {
    const _animate = function() {
        requestAnimationFrame(_animate);
        const dt = state.clock.getDelta();
        const t = state.clock.getElapsedTime();

        if (state.autoRotate && state.bellGroup) {
            state.bellGroup.rotation.y += dt * 0.3;
        }

        if (state.particles) {
            state.particles.rotation.y += dt * 0.02;
            const pos = state.particles.geometry.attributes.position.array;
            for (let i = 0; i < pos.length / 3; i++) {
                pos[i*3+1] += Math.sin(t * 0.5 + i) * 0.002;
            }
            state.particles.geometry.attributes.position.needsUpdate = true;
        }

        if (state.bellGroup && (state.vibrationAmp > 0 || state.strikeIntensity > 0)) {
            const vib = state.vibrationAmp + state.strikeIntensity;
            state.bellGroup.rotation.z = Math.sin(t * 30) * vib * 0.05;
            state.bellGroup.position.x = Math.sin(t * 25) * vib * 0.01;
            state.strikeIntensity *= Math.pow(0.001, dt);
            state.vibrationAmp = Math.max(0, state.vibrationAmp - dt * 0.3);
        }

        state.defectMeshes.forEach((m, i) => {
            m.scale.setScalar(1 + Math.sin(t * 2 + i) * 0.15);
            m.material.opacity = 0.5 + Math.sin(t * 3 + i) * 0.25;
        });

        if (state.viewMode === 'sound') {
            if ((animate._soundTimer || 0) + 50 < t * 1000) {
                drawSoundField();
                animate._soundTimer = t * 1000;
            }
        }

        state.controls.update();
        state.renderer.render(state.scene, state.camera);
    };
    _animate();
    return animate;
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
    state.vibrationAmp = r.acoustic_amplitude;
    renderFreqBars(r.acoustic_harmonics || []);
}

function renderFreqBars(harmonics) {
    const container = $('freq-bars');
    container.innerHTML = '';
    if (!harmonics.length) return;
    const max = Math.max(...harmonics);
    harmonics.forEach((h, i) => {
        const bar = document.createElement('div');
        bar.className = 'bar';
        const rel = max > 0 ? (h / (max * (1 + i * 0.5))) : 0;
        bar.style.height = Math.max(2, Math.round(rel * 100)) + '%';
        bar.title = `${h.toFixed(1)}Hz`;
        container.appendChild(bar);
    });
}

async function loadSimulations(bellId) {
    const [cast, acou] = await Promise.all([
        api(`/sim/casting/bell/${bellId}?limit=1`),
        api(`/sim/acoustic/bell/${bellId}?limit=1`),
    ]);
    if (cast.success && cast.data?.[0]) {
        state.currentCastingSim = cast.data[0];
        if (state.viewMode === 'defect') visualizeDefects(cast.data[0]);
    }
    if (acou.success && acou.data?.[0]) {
        state.currentAcousticSim = acou.data[0];
        const freqs = acou.data[0].natural_frequencies;
        if (freqs?.length) renderFreqBars(freqs);
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

        $('main-canvas').style.display = (state.viewMode === 'sound') ? 'none' : 'block';
        $('sound-field-canvas').style.display = (state.viewMode === 'sound') ? 'block' : 'none';

        if (state.viewMode === 'defect' && state.currentCastingSim) {
            visualizeDefects(state.currentCastingSim);
        } else {
            state.defectMeshes.forEach(m => state.scene.remove(m));
            state.defectMeshes = [];
        }
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
            if (state.viewMode === 'defect') visualizeDefects(res.data);
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
            const freqs = res.data.natural_frequencies;
            if (freqs?.length) renderFreqBars(freqs);
            alert(`声学仿真完成！\n音准: ${res.data.pitch_ok ? '合格' : '偏差'} (${res.data.pitch_deviation_cents.toFixed(1)}音分)\n基频: ${freqs?.[0]?.toFixed(2) || '-'}Hz\n声功率: ${res.data.sound_power.toFixed(4)}W`);
        }
    };

    $('btn-strike').onclick = () => {
        state.strikeIntensity = 1.5;
        playBellSound();
    };

    $('btn-rotate').onclick = () => {
        state.autoRotate = !state.autoRotate;
        $('btn-rotate').textContent = state.autoRotate ? '⏸ 停止旋转' : '⟳ 自动旋转';
    };

    $('btn-sim-all').onclick = async () => {
        if (!state.selectedBell) return;
        state.viewMode = 'casting';
        document.querySelectorAll('.view-tab').forEach(t => {
            t.classList.toggle('active', t.dataset.view === 'casting');
        });
        $('main-canvas').style.display = 'block';
        $('sound-field-canvas').style.display = 'none';
        runCastingAnimation();
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
    initThree();
    bindEvents();
    initClock();
    await loadBells();
    loadAlerts();
    setInterval(loadAlerts, 10000);
    setInterval(() => {
        if (state.selectedBell) loadLatestSensorData(state.selectedBell.bell_id);
    }, 5000);
    console.log('🚀 古代铸钟仿真系统前端已加载');
}

init();
