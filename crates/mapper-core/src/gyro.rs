use glam::{Vec2, Vec3};
use std::f32::consts::PI;

/// Low-Pass Filter sederhana sebagai komponen dasar One Euro Filter
struct LowPassFilter {
    y: f32,
    initialized: bool,
}

impl LowPassFilter {
    fn new() -> Self {
        Self { y: 0.0, initialized: false }
    }

    fn filter(&mut self, value: f32, alpha: f32) -> f32 {
        if !self.initialized {
            self.y = value;
            self.initialized = true;
        } else {
            self.y = alpha * value + (1.0 - alpha) * self.y;
        }
        self.y
    }
}

/// Implementasi One Euro Filter (1€ Filter) untuk 1 sumbu
pub struct OneEuroFilter1D {
    min_cutoff: f32, // Frekuensi cutoff minimum (Hz) -> Makin kecil = makin halus saat diam
    beta: f32,       // Koefisien kecepatan -> Makin besar = makin sedikit lag saat gerak cepat
    d_cutoff: f32,   // Frekuensi cutoff untuk turunan (kecepatan)
    x_filter: LowPassFilter,
    dx_filter: LowPassFilter,
    prev_x: Option<f32>,
}

impl OneEuroFilter1D {
    pub fn new(min_cutoff: f32, beta: f32) -> Self {
        Self {
            min_cutoff,
            beta,
            d_cutoff: 1.0,
            x_filter: LowPassFilter::new(),
            dx_filter: LowPassFilter::new(),
            prev_x: None,
        }
    }

    fn compute_alpha(cutoff: f32, dt: f32) -> f32 {
        let tau = 1.0 / (2.0 * PI * cutoff);
        1.0 / (1.0 + tau / dt)
    }

    pub fn filter(&mut self, x: f32, dt: f32) -> f32 {
        // Hindari pembagian nol jika dt terlalu kecil
        let dt = dt.max(0.0001);

        // Hitung kecepatan perubahan sinyal (derivative)
        let dx = match self.prev_x {
            Some(prev) => (x - prev) / dt,
            None => 0.0,
        };
        self.prev_x = Some(x);

        // Haluskan nilai kecepatan
        let alpha_d = Self::compute_alpha(self.d_cutoff, dt);
        let edx = self.dx_filter.filter(dx, alpha_d);

        // Sesuaikan frekuensi cutoff secara dinamis berdasarkan kecepatan gerak
        let cutoff = self.min_cutoff + self.beta * edx.abs();
        let alpha = Self::compute_alpha(cutoff, dt);

        self.x_filter.filter(x, alpha)
    }
}

/// Kalibrator otomatis untuk menghilangkan Gyro Drift saat kontroler diam
pub struct GyroCalibrator {
    samples_needed: usize,
    accumulated: Vec3,
    sample_count: usize,
    pub bias: Vec3,
    pub is_calibrated: bool,
}

impl GyroCalibrator {
    pub fn new(samples_needed: usize) -> Self {
        Self {
            samples_needed,
            accumulated: Vec3::ZERO,
            sample_count: 0,
            bias: Vec3::ZERO,
            is_calibrated: false,
        }
    }

    /// Masukkan data gyro mentah. Akan mengembalikan `true` jika kalibrasi selesai.
    pub fn update(&mut self, raw_gyro: Vec3, still_threshold: f32) -> bool {
        if self.is_calibrated {
            return true;
        }

        // Pastikan kontroler benar-benar diam sebelum mengambil sampel
        if raw_gyro.length() > still_threshold {
            // Reset jika kontroler tiba-tiba digerakkan saat kalibrasi
            self.accumulated = Vec3::ZERO;
            self.sample_count = 0;
            return false;
        }

        self.accumulated += raw_gyro;
        self.sample_count += 1;

        if self.sample_count >= self.samples_needed {
            self.bias = self.accumulated / (self.sample_count as f32);
            self.is_calibrated = true;
        }

        self.is_calibrated
    }

    pub fn reset(&mut self) {
        self.accumulated = Vec3::ZERO;
        self.sample_count = 0;
        self.bias = Vec3::ZERO;
        self.is_calibrated = false;
    }
}

/// Konfigurasi lengkap Gyro-to-Mouse ala Steam Input
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct GyroConfig {
    pub sensitivity_x: f32,
    pub sensitivity_y: f32,
    pub min_cutoff: f32, // Rekomendasi: 0.5 - 1.5
    pub beta: f32,       // Rekomendasi: 0.01 - 0.05
    pub invert_y: bool,
}

impl Default for GyroConfig {
    fn default() -> Self {
        Self {
            sensitivity_x: 1.0,
            sensitivity_y: 1.0,
            min_cutoff: 1.0,
            beta: 0.02,
            invert_y: false,
        }
    }
}

/// Pipeline utama pemroses Gyro menjadi pergerakan kamera 2D (Mouse / Right Stick)
pub struct GyroProcessor {
    pub calibrator: GyroCalibrator,
    filter_yaw: OneEuroFilter1D,
    filter_pitch: OneEuroFilter1D,
    pub config: GyroConfig,
    pub active: bool, // Untuk fitur Gyro Ratcheting (tombol on/off sementara)
}

impl GyroProcessor {
    pub fn new(config: GyroConfig) -> Self {
        let filter_yaw = OneEuroFilter1D::new(config.min_cutoff, config.beta);
        let filter_pitch = OneEuroFilter1D::new(config.min_cutoff, config.beta);

        Self {
            // 500 sampel pada polling 500Hz = kalibrasi 1 detik
            calibrator: GyroCalibrator::new(500),
            filter_yaw,
            filter_pitch,
            config,
            active: true,
        }
    }

    /// Proses data gyro 3D (rad/s) menjadi delta mouse 2D (x, y)
    pub fn process(&mut self, raw_gyro: Vec3, dt: f32) -> Vec2 {
        // 1. Jika belum terkalibrasi, kumpulkan sampel bias terlebih dahulu
        if !self.calibrator.is_calibrated {
            self.calibrator.update(raw_gyro, 0.05);
            return Vec2::ZERO;
        }

        // 2. Kurangi nilai bias agar titik diam benar-benar 0.0
        let calibrated = raw_gyro - self.calibrator.bias;

        // 3. Jika Gyro sedang dinonaktifkan sementara (Ratcheting), kembalikan nol
        if !self.active {
            return Vec2::ZERO;
        }

        // Pemetaan standar kontroler (Yaw = sumbu Y/Z tergantung orientasi, Pitch = sumbu X)
        // Di sini menggunakan konvensi umum SDL: X = Pitch (atas/bawah), Y = Yaw (kiri/kanan)
        let smooth_yaw = self.filter_yaw.filter(-calibrated.y, dt);
        let smooth_pitch = self.filter_pitch.filter(-calibrated.x, dt);

        let out_x = smooth_yaw * self.config.sensitivity_x;
        let mut out_y = smooth_pitch * self.config.sensitivity_y;

        if self.config.invert_y {
            out_y = -out_y;
        }

        Vec2::new(out_x, out_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gyro_calibration_removes_drift() {
        let mut processor = GyroProcessor::new(GyroConfig::default());
        let drift = Vec3::new(0.015, -0.02, 0.005); // Simulasi sensor cacat/drift

        // Simulasikan 500 frame saat kontroler diam
        for _ in 0..500 {
            let out = processor.process(drift, 0.002);
            assert_eq!(out, Vec2::ZERO);
        }

        assert!(processor.calibrator.is_calibrated);

        // Setelah terkalibrasi, input sebesar 'drift' harus menghasilkan output 0
        let output_when_still = processor.process(drift, 0.002);
        assert!(output_when_still.length() < 0.0001);
    }

    #[test]
    fn test_one_euro_filter_smooths_jitter_but_tracks_fast_motion() {
        let mut filter = OneEuroFilter1D::new(1.0, 0.05);
        let dt = 0.002; // 500Hz

        // Inisialisasi titik diam awal di 0.0
        filter.filter(0.0, dt);

        // Simulasi 30 frame getaran tangan frekuensi tinggi (+/- 0.02 rad/s)
        let mut max_jitter_out: f32 = 0.0;
        for i in 0..30 {
            let val = if i % 2 == 0 { 0.02 } else { -0.02 };
            let out = filter.filter(val, dt);
            max_jitter_out = max_jitter_out.max(out.abs());
        }
        // Output getaran harus teredam jauh lebih kecil dari puncak input (0.02)
        assert!(max_jitter_out < 0.005, "max_jitter_out was {}", max_jitter_out);

        let mut last_out = 0.0;
        // Simulasi gerakan menoleh cepat (5.0 rad/s)
        for _ in 0..50 {
            last_out = filter.filter(5.0, dt);
        }
        // Filter harus cepat mengejar nilai target tanpa lag berat
        assert!(last_out > 4.5);
    }
}
