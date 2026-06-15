import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

export class Bell3D {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.scene = null;
        this.camera = null;
        this.renderer = null;
        this.controls = null;
        this.bellGroup = null;
        this.particles = null;
        this.moltenMetal = null;
        this.defectMeshes = [];
        this.clock = new THREE.Clock();
        this.vibrationAmp = 0;
        this.strikeIntensity = 0;
        this.autoRotate = false;
        this.currentBell = null;
        this.currentCastingSim = null;
        this.castingStage = null;
        this.viewMode = '3d';
        this._tempMold = null;
        this._tempMetal = null;
        this._soundTimer = 0;
        this._animate = this._animate.bind(this);
    }

    init() {
        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x05080f);
        this.scene.fog = new THREE.Fog(0x05080f, 15, 40);

        const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
        this.camera = new THREE.PerspectiveCamera(45, w / h, 0.1, 1000);
        this.camera.position.set(3, 2.5, 5);

        this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, antialias: true });
        this.renderer.setPixelRatio(window.devicePixelRatio);
        this.renderer.setSize(w, h, false);
        this.renderer.shadowMap.enabled = true;
        this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
        this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
        this.renderer.toneMappingExposure = 1.1;

        this.controls = new OrbitControls(this.camera, this.canvas);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.08;
        this.controls.minDistance = 2;
        this.controls.maxDistance = 20;
        this.controls.target.set(0, 0, 0);

        const ambient = new THREE.AmbientLight(0x404060, 0.6);
        this.scene.add(ambient);

        const keyLight = new THREE.DirectionalLight(0xffe5b3, 1.0);
        keyLight.position.set(5, 8, 5);
        keyLight.castShadow = true;
        keyLight.shadow.mapSize.set(1024, 1024);
        this.scene.add(keyLight);

        const rimLight = new THREE.DirectionalLight(0x8888ff, 0.4);
        rimLight.position.set(-5, 3, -5);
        this.scene.add(rimLight);

        const fillLight = new THREE.PointLight(0xe8c468, 0.8, 20);
        fillLight.position.set(0, 2, 0);
        this.scene.add(fillLight);

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
        this.scene.add(floor);

        this._addFloorPattern();
        this._addParticles();

        window.addEventListener('resize', () => this._onResize());
        this._animate();
    }

    _addFloorPattern() {
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
        this.scene.add(group);
    }

    _addParticles() {
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
        this.particles = new THREE.Points(geo, mat);
        this.scene.add(this.particles);
    }

    _onResize() {
        const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
        this.camera.aspect = w / h;
        this.camera.updateProjectionMatrix();
        this.renderer.setSize(w, h, false);
    }

    setViewMode(mode) {
        this.viewMode = mode;
        this.canvas.style.display = (mode === 'sound') ? 'none' : 'block';
        if (mode !== 'defect') {
            this.defectMeshes.forEach(m => this.scene.remove(m));
            this.defectMeshes = [];
        } else if (this.currentCastingSim) {
            this.visualizeDefects(this.currentCastingSim);
        }
    }

    toggleAutoRotate() {
        this.autoRotate = !this.autoRotate;
        return this.autoRotate;
    }

    strike() {
        this.strikeIntensity = 1.5;
    }

    setVibrationAmp(amp) {
        this.vibrationAmp = amp;
    }

    buildBellMesh(bell) {
        if (this.bellGroup) this.scene.remove(this.bellGroup);
        this.defectMeshes.forEach(m => this.scene.remove(m));
        this.defectMeshes = [];
        if (this.moltenMetal) { this.scene.remove(this.moltenMetal); this.moltenMetal = null; }

        this.currentBell = bell;
        this.bellGroup = new THREE.Group();
        const scale = Math.max(0.8, Math.min(3, bell.height_m));
        this.bellGroup.scale.setScalar(scale);

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
        this.bellGroup.add(bellMesh);

        const knobGeo = new THREE.SphereGeometry(0.12, 24, 24);
        const knobMat = new THREE.MeshStandardMaterial({
            color: 0xd4af37,
            metalness: 0.95,
            roughness: 0.2,
        });
        const knob = new THREE.Mesh(knobGeo, knobMat);
        knob.position.y = height / 2 + 0.1;
        knob.castShadow = true;
        this.bellGroup.add(knob);

        const crownGeo = new THREE.TorusGeometry(0.18, 0.025, 12, 32);
        const crown = new THREE.Mesh(crownGeo, knobMat);
        crown.position.y = height / 2 + 0.22;
        crown.rotation.x = Math.PI / 2;
        crown.castShadow = true;
        this.bellGroup.add(crown);

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
        this.bellGroup.add(rim);

        this._addDecorativeRings(this.bellGroup, height, midR);
        this._addInscriptions(this.bellGroup, height, midR);

        this.bellGroup.position.y = 0;
        this.scene.add(this.bellGroup);

        const fitDist = 3 / scale + 2;
        this.camera.position.set(fitDist, fitDist * 0.6, fitDist);
        this.controls.target.set(0, 0, 0);
        this.controls.update();
    }

    _addDecorativeRings(group, height, midR) {
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

    _addInscriptions(group, height, midR) {
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

    runCastingAnimation() {
        if (!this.currentBell) return;
        document.getElementById('casting-stage-bar').style.display = 'block';

        const stages = [
            { name: '制模阶段', key: 'molding', dur: 1500, func: (d) => this._animateMolding(d) },
            { name: '熔炼阶段', key: 'melting', dur: 2000, func: (d) => this._animateMelting(d) },
            { name: '浇注阶段', key: 'pouring', dur: 3000, func: (d) => this._animatePouring(d) },
            { name: '冷却阶段', key: 'cooling', dur: 2500, func: (d) => this._animateCooling(d) },
            { name: '凝固阶段', key: 'solidifying', dur: 2500, func: (d) => this._animateSolidifying(d) },
            { name: '铸造完成', key: 'finished', dur: 1000, func: (d) => this._animateFinished(d) },
        ];

        const totalDur = stages.reduce((s, x) => s + x.dur, 0);
        let elapsed = 0;

        stages.forEach((stage, idx) => {
            const stageStart = elapsed;
            setTimeout(() => {
                document.getElementById('stage-name').textContent = stage.name;
                const overallPct = Math.round((stageStart / totalDur) * 100);
                document.getElementById('progress-fill').style.width = overallPct + '%';
                document.getElementById('stage-progress').textContent = overallPct + '%';
                this.castingStage = stage.key;
                stage.func(stage.dur);
            }, stageStart);
            elapsed += stage.dur;
        });

        setTimeout(() => {
            document.getElementById('progress-fill').style.width = '100%';
            document.getElementById('stage-progress').textContent = '100%';
        }, totalDur);
    }

    _animateMolding(dur) {
        if (this.bellGroup) this.bellGroup.visible = false;
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
        this.scene.add(mold);
        this._tempMold = mold;

        let t = 0;
        const step = () => {
            t += 16;
            const p = Math.min(1, t / dur);
            mold.material.opacity = 0.4 + p * 0.4;
            if (p < 1) requestAnimationFrame(step);
        };
        step();
    }

    _animateMelting(dur) {
        const metalGroup = new THREE.Group();
        this.scene.add(metalGroup);
        this._tempMetal = metalGroup;

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

    _animatePouring(dur) {
        this.moltenMetal = this._tempMetal;

        const streamGeo = new THREE.CylinderGeometry(0.05, 0.08, 3, 16);
        const streamMat = new THREE.MeshStandardMaterial({
            color: 0xff3300,
            emissive: 0xff4400,
            emissiveIntensity: 3,
        });
        const stream = new THREE.Mesh(streamGeo, streamMat);
        stream.position.set(0, 0.5, 1.2);
        stream.rotation.z = Math.PI / 8;
        this.scene.add(stream);

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
        this.scene.add(fill);

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
                this.scene.remove(stream);
                this.scene.remove(fill);
            }
        };
        step();
    }

    _animateCooling(dur) {
        const mold = this._tempMold;
        let t = 0;
        const step = () => {
            t += 16;
            const p = Math.min(1, t / dur);
            if (mold) mold.material.opacity = 0.8 * (1 - p);
            if (p < 1) requestAnimationFrame(step);
            else {
                if (mold) this.scene.remove(mold);
                if (this.bellGroup) {
                    this.bellGroup.visible = true;
                    this.bellGroup.children.forEach(c => {
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

    _animateSolidifying(dur) {
        let t = 0;
        const step = () => {
            t += 16;
            const p = Math.min(1, t / dur);
            if (this.bellGroup) {
                this.bellGroup.children.forEach(c => {
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

    _animateFinished(dur) {
        if (this.moltenMetal) {
            this.scene.remove(this.moltenMetal);
            this.moltenMetal = null;
        }
        if (this.bellGroup) {
            this.bellGroup.children.forEach(c => {
                if (c.material && c.material.emissive) c.material.emissiveIntensity = 0;
            });
        }
    }

    visualizeDefects(sim) {
        this.currentCastingSim = sim;
        this.defectMeshes.forEach(m => this.scene.remove(m));
        this.defectMeshes = [];
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
            const bell = this.currentBell;
            const s = Math.max(0.8, Math.min(3, bell.height_m));
            mesh.position.set(
                (x - 0.5) * 2 * s,
                (y - 0.5) * 2 * s,
                (z - 0.5) * 2 * s
            );
            this.scene.add(mesh);
            this.defectMeshes.push(mesh);
        });
    }

    _animate() {
        requestAnimationFrame(this._animate);
        const dt = this.clock.getDelta();
        const t = this.clock.getElapsedTime();

        if (this.autoRotate && this.bellGroup) {
            this.bellGroup.rotation.y += dt * 0.3;
        }

        if (this.particles) {
            this.particles.rotation.y += dt * 0.02;
            const pos = this.particles.geometry.attributes.position.array;
            for (let i = 0; i < pos.length / 3; i++) {
                pos[i*3+1] += Math.sin(t * 0.5 + i) * 0.002;
            }
            this.particles.geometry.attributes.position.needsUpdate = true;
        }

        if (this.bellGroup && (this.vibrationAmp > 0 || this.strikeIntensity > 0)) {
            const vib = this.vibrationAmp + this.strikeIntensity;
            this.bellGroup.rotation.z = Math.sin(t * 30) * vib * 0.05;
            this.bellGroup.position.x = Math.sin(t * 25) * vib * 0.01;
            this.strikeIntensity *= Math.pow(0.001, dt);
            this.vibrationAmp = Math.max(0, this.vibrationAmp - dt * 0.3);
        }

        this.defectMeshes.forEach((m, i) => {
            m.scale.setScalar(1 + Math.sin(t * 2 + i) * 0.15);
            m.material.opacity = 0.5 + Math.sin(t * 3 + i) * 0.25;
        });

        this.controls.update();
        this.renderer.render(this.scene, this.camera);
    }
}
