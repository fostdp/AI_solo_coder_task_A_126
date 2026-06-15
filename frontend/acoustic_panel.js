export class AcousticPanel {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.soundFieldWorker = null;
        this.soundFieldPending = false;
        this.soundFieldReqId = 0;
        this.currentAcousticSim = null;
        this.selectedBell = null;
        this._currentFreq = 0;
        this._currentCx = 0;
        this._currentCy = 0;
        this._currentW = 0;
        this._currentH = 0;
        this._soundTimer = 0;
        this._animate = this._animate.bind(this);
        this._running = false;
    }

    init() {
        this._initSoundFieldWorker();
        this._running = true;
        this._animate();
    }

    destroy() {
        this._running = false;
        if (this.soundFieldWorker) {
            this.soundFieldWorker.terminate();
            this.soundFieldWorker = null;
        }
    }

    setBell(bell) {
        this.selectedBell = bell;
    }

    setAcousticSim(sim) {
        this.currentAcousticSim = sim;
    }

    setViewMode(mode) {
        this.canvas.style.display = (mode === 'sound') ? 'block' : 'none';
        if (mode === 'sound') {
            this.drawSoundField();
        }
    }

    _initSoundFieldWorker() {
        if (typeof Worker === 'undefined') {
            console.warn('WebWorker not supported, falling back to main thread rendering');
            this.soundFieldWorker = null;
            return;
        }

        try {
            this.soundFieldWorker = new Worker('sound_field_worker.js');
            this.soundFieldWorker.onmessage = (e) => {
                const { type, id, imageData, width, height, renderTime } = e.data;
                if (type === 'result' && this.canvas.style.display !== 'none') {
                    const dpr = window.devicePixelRatio || 1;
                    this.canvas.width = width;
                    this.canvas.height = height;

                    const imgData = new ImageData(
                        new Uint8ClampedArray(imageData),
                        width / dpr,
                        height / dpr
                    );

                    this.ctx.putImageData(imgData, 0, 0);
                    this._drawOverlay(width / dpr, height / dpr);

                    if (renderTime > 16) {
                        console.debug(`[SoundField] rendered in ${renderTime.toFixed(1)}ms`);
                    }
                }
                this.soundFieldPending = false;
            };

            this.soundFieldWorker.onerror = (e) => {
                console.error('SoundField Worker error:', e);
                this.soundFieldWorker = null;
                this.soundFieldPending = false;
            };

            console.log('✅ SoundField WebWorker initialized');
        } catch (e) {
            console.warn('Failed to init WebWorker, falling back:', e);
            this.soundFieldWorker = null;
        }
    }

    drawSoundField() {
        const dpr = window.devicePixelRatio || 1;
        const w = this.canvas.clientWidth;
        const h = this.canvas.clientHeight;

        const freq = this.currentAcousticSim
            ? this.currentAcousticSim.natural_frequencies?.[0] || 261.63
            : this.selectedBell?.expected_freq_hz || 261.63;
        const cx = w / 2;
        const cy = h * 0.35;
        const srcStrength = 1 / Math.max(20, this.selectedBell?.weight_kg || 50);

        this._currentFreq = freq;
        this._currentCx = cx;
        this._currentCy = cy;
        this._currentW = w;
        this._currentH = h;

        if (this.soundFieldWorker && !this.soundFieldPending) {
            this.soundFieldReqId++;
            this.soundFieldPending = true;

            this.soundFieldWorker.postMessage({
                type: 'compute',
                id: this.soundFieldReqId,
                params: {
                    width: Math.round(w * dpr),
                    height: Math.round(h * dpr),
                    freq,
                    cx: cx * dpr,
                    cy: cy * dpr,
                    srcStrength,
                    time: performance.now(),
                },
            });
        } else if (!this.soundFieldWorker) {
            this._drawFallback();
        }
    }

    _drawFallback() {
        const dpr = window.devicePixelRatio || 1;
        const w = this.canvas.clientWidth;
        const h = this.canvas.clientHeight;
        this.canvas.width = w * dpr;
        this.canvas.height = h * dpr;
        this.ctx.scale(dpr, dpr);

        this.ctx.clearRect(0, 0, w, h);

        const freq = this.currentAcousticSim
            ? this.currentAcousticSim.natural_frequencies?.[0] || 261.63
            : this.selectedBell?.expected_freq_hz || 261.63;

        const soundSpeed = 343;
        const lambda = soundSpeed / freq;
        const k = 2 * Math.PI / lambda;

        const cx = w / 2;
        const cy = h * 0.35;

        const imgData = this.ctx.createImageData(w, h);
        const data = imgData.data;
        const srcStrength = 1 / Math.max(20, this.selectedBell?.weight_kg || 50);

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
                    const [rr, gg, bb] = this._hsl2rgb(hue, 0.85, 0.4 + pressure * 0.3);
                    data[idx] = rr; data[idx+1] = gg; data[idx+2] = bb; data[idx+3] = 255;
                }
            }
        }
        this.ctx.putImageData(imgData, 0, 0);
        this._drawOverlay(w, h);
    }

    _drawOverlay(w, h) {
        const freq = this._currentFreq || 261.63;
        const cx = this._currentCx || w / 2;
        const cy = this._currentCy || h * 0.35;
        const lambda = 343 / freq;

        const grad = this.ctx.createRadialGradient(cx, cy, 5, cx, cy, 30);
        grad.addColorStop(0, 'rgba(255,220,120,0.9)');
        grad.addColorStop(0.5, 'rgba(232,196,104,0.5)');
        grad.addColorStop(1, 'rgba(232,196,104,0)');
        this.ctx.fillStyle = grad;
        this.ctx.beginPath();
        this.ctx.arc(cx, cy, 30, 0, Math.PI * 2);
        this.ctx.fill();

        this.ctx.fillStyle = '#e8c468';
        this.ctx.font = 'bold 14px "Microsoft YaHei"';
        this.ctx.fillText(`声压级云图 · ${freq.toFixed(1)}Hz · λ=${lambda.toFixed(2)}m`, 20, 30);
        this.ctx.font = '11px "Microsoft YaHei"';
        this.ctx.fillStyle = '#8892b0';
        this.ctx.fillText('图例: 蓝(低) → 青 → 绿 → 黄 → 红(高)', 20, 50);

        this._drawColorBar(w - 40, 60, 20, h - 120);
    }

    _drawColorBar(x, y, w, h) {
        for (let i = 0; i < h; i++) {
            const t = 1 - i / h;
            const hue = (1 - t) * 0.65;
            const [r, g, b] = this._hsl2rgb(hue, 0.85, 0.4 + t * 0.3);
            this.ctx.fillStyle = `rgb(${r},${g},${b})`;
            this.ctx.fillRect(x, y + i, w, 1);
        }
        this.ctx.strokeStyle = '#2a3552';
        this.ctx.lineWidth = 1;
        this.ctx.strokeRect(x, y, w, h);
        this.ctx.fillStyle = '#8892b0';
        this.ctx.font = '10px monospace';
        this.ctx.textAlign = 'left';
        this.ctx.fillText('高', x + w + 6, y + 8);
        this.ctx.fillText('低', x + w + 6, y + h);
    }

    _hsl2rgb(h, s, l) {
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

    renderFreqBars(harmonics, containerId = 'freq-bars') {
        const container = document.getElementById(containerId);
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

    _animate() {
        if (!this._running) return;
        requestAnimationFrame(this._animate);

        const t = performance.now();
        if (this.canvas.style.display !== 'none' &&
            (this._soundTimer || 0) + 50 < t) {
            this.drawSoundField();
            this._soundTimer = t;
        }
    }
}
