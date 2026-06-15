/**
 * 声场云图WebWorker
 * 负责在后台线程计算声场像素数据，避免阻塞主线程（移动端卡顿问题）
 */

const SPEED_OF_SOUND = 343;
const P_REF = 2e-5;

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

function computeSoundField(params) {
    const { width, height, freq, cx, cy, srcStrength, time } = params;
    const lambda = SPEED_OF_SOUND / freq;
    const k = 2 * Math.PI / lambda;

    const buffer = new Uint8ClampedArray(width * height * 4);

    for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
            const dx = (x - cx) / width * 15;
            const dy = (y - cy) / height * 15;
            const r = Math.sqrt(dx * dx + dy * dy);
            const theta = Math.atan2(dy, dx);

            let pressure;
            if (r < 0.2) {
                pressure = 1;
            } else {
                const directivity = 1 + 0.6 * Math.pow(Math.cos(theta - Math.PI / 2), 2);
                const wave = Math.sin(k * r * 0.3 - time * 0.002) / Math.sqrt(r + 0.1);
                pressure = Math.abs(wave * directivity * srcStrength * 500);
            }

            pressure = Math.min(1, pressure / 1.5);
            const idx = (y * width + x) * 4;

            if (pressure < 0.01) {
                buffer[idx] = 5;
                buffer[idx + 1] = 8;
                buffer[idx + 2] = 15;
                buffer[idx + 3] = 255;
            } else {
                const hue = (1 - pressure) * 0.65;
                const [rr, gg, bb] = hsl2rgb(hue, 0.85, 0.4 + pressure * 0.3);
                buffer[idx] = rr;
                buffer[idx + 1] = gg;
                buffer[idx + 2] = bb;
                buffer[idx + 3] = 255;
            }
        }
    }

    return buffer;
}

self.onmessage = (e) => {
    const { type, params, id } = e.data;

    if (type === 'compute') {
        const t0 = performance.now();
        const imageData = computeSoundField(params);
        const t1 = performance.now();
        self.postMessage(
            { type: 'result', id, imageData, width: params.width, height: params.height, renderTime: t1 - t0 },
            [imageData.buffer]
        );
    }
};
