use std::path::Path;
use std::sync::OnceLock;
use log::info;
use serde::{Deserialize, Serialize};

/// Detailed hardware recommendations DTO for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareRecommendation {
    pub cpu_cores: u8,
    pub memory_gb: u8,
    pub gpu_type: String,
    pub has_gpu_acceleration: bool,
    pub performance_tier: String, // "Low", "Medium", "High", "Ultra"
    pub recommended_transcription_engine: String, // e.g. "Parakeet TDT 0.6B (Metal/GPU)" or "Whisper Tiny (CPU)"
    pub recommended_transcription_model: String,
    pub recommended_summary_provider: String, // e.g. "Built-in AI (qwen3.5:4b)" or "Cloud Provider"
    pub recommended_summary_model: String,
    pub max_recommended_context: usize,
    pub explanation: String,
}

/// Hardware capabilities for audio processing optimization
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_cores: u8,
    pub has_gpu_acceleration: bool,
    pub gpu_type: GpuType,
    pub memory_gb: u8,
    pub performance_tier: PerformanceTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuType {
    None,
    Metal,      // Apple Silicon
    Cuda,       // NVIDIA
    Vulkan,     // AMD/Intel
    OpenCL,     // Generic GPU compute
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceTier {
    Low,      // CPU-only, limited resources
    Medium,   // CPU-only but powerful, or basic GPU
    High,     // Dedicated GPU with good compute
    Ultra,    // High-end hardware with fast GPU
}

/// Adaptive Whisper configuration based on hardware
#[derive(Debug, Clone)]
pub struct AdaptiveWhisperConfig {
    pub beam_size: usize,
    pub temperature: f32,
    pub use_gpu: bool,
    pub max_threads: Option<usize>,
    pub chunk_size_preference: ChunkSizePreference,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkSizePreference {
    Fast,       // Smaller chunks for responsiveness
    Balanced,   // Medium chunks for balance
    Quality,    // Larger chunks for accuracy
}

static HARDWARE_PROFILE: OnceLock<HardwareProfile> = OnceLock::new();

impl HardwareProfile {
    /// Get the detected hardware profile (cached after first call)
    pub fn detect() -> &'static HardwareProfile {
        HARDWARE_PROFILE.get_or_init(|| {
            let profile = Self::detect_hardware();
            info!("Detected hardware profile: {:?}", profile);
            profile
        })
    }

    /// Perform hardware detection
    fn detect_hardware() -> HardwareProfile {
        let cpu_cores = Self::detect_cpu_cores();
        let (has_gpu_acceleration, gpu_type) = Self::detect_gpu();
        let memory_gb = Self::detect_memory_gb();
        let performance_tier = Self::calculate_performance_tier(cpu_cores, &gpu_type, memory_gb);

        HardwareProfile {
            cpu_cores,
            has_gpu_acceleration,
            gpu_type,
            memory_gb,
            performance_tier,
        }
    }

    /// Detect number of CPU cores
    fn detect_cpu_cores() -> u8 {
        std::thread::available_parallelism()
            .map(|n| n.get().min(255) as u8)
            .unwrap_or(4) // Default to 4 cores
    }

    /// Detect GPU acceleration capabilities
    fn detect_gpu() -> (bool, GpuType) {
        // Check for Metal (Apple Silicon)
        #[cfg(target_os = "macos")]
        {
            if Self::has_metal_support() {
                return (true, GpuType::Metal);
            }
        }

        // Check for CUDA (NVIDIA)
        if Self::has_cuda_support() {
            return (true, GpuType::Cuda);
        }

        // Check for Vulkan (AMD/Intel/others)
        if Self::has_vulkan_support() {
            return (true, GpuType::Vulkan);
        }

        // Fallback to CPU-only
        (false, GpuType::None)
    }

    /// Detect available system memory in GB
    fn detect_memory_gb() -> u8 {
        // Simple memory detection - could be enhanced with system-specific calls
        match std::env::var("MEMORY_GB") {
            Ok(mem_str) => mem_str.parse().unwrap_or(8),
            Err(_) => {
                // Default estimates based on common configurations
                8 // Conservative default
            }
        }
    }

    /// Calculate performance tier based on hardware
    fn calculate_performance_tier(cpu_cores: u8, gpu_type: &GpuType, memory_gb: u8) -> PerformanceTier {
        match gpu_type {
            GpuType::Metal => {
                if memory_gb >= 16 && cpu_cores >= 8 {
                    PerformanceTier::Ultra
                } else {
                    PerformanceTier::High
                }
            }
            GpuType::Cuda => {
                if memory_gb >= 16 && cpu_cores >= 8 {
                    PerformanceTier::Ultra
                } else {
                    PerformanceTier::High
                }
            }
            GpuType::Vulkan | GpuType::OpenCL => {
                if memory_gb >= 12 && cpu_cores >= 6 {
                    PerformanceTier::High
                } else {
                    PerformanceTier::Medium
                }
            }
            GpuType::None => {
                if cpu_cores >= 8 && memory_gb >= 16 {
                    PerformanceTier::Medium
                } else {
                    PerformanceTier::Low
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn has_metal_support() -> bool {
        // Simple check for Apple Silicon (Metal is available on Intel Macs too, but less optimal for ML)
        std::env::consts::ARCH == "aarch64"
    }

    fn has_cuda_support() -> bool {
        // Check for CUDA environment or libraries
        std::env::var("CUDA_PATH").is_ok() ||
        std::env::var("CUDA_HOME").is_ok() ||
        std::path::Path::new("/usr/local/cuda").exists()
    }

    fn has_vulkan_support() -> bool {
        if std::env::var("VULKAN_SDK").is_ok() ||
            std::path::Path::new("/usr/lib/x86_64-linux-gnu/libvulkan.so").exists() ||
            std::path::Path::new("/usr/lib/libvulkan.so").exists()
        {
            return true;
        }

        #[cfg(target_os = "windows")]
        {
            return Self::has_windows_vulkan_runtime();
        }

        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    #[cfg(target_os = "windows")]
    fn has_windows_vulkan_runtime() -> bool {
        for env_var in ["SystemRoot", "WINDIR"] {
            if let Ok(system_root) = std::env::var(env_var) {
                if Self::has_windows_vulkan_loader(Path::new(&system_root)) {
                    return true;
                }
            }
        }

        Self::has_windows_vulkan_loader(Path::new(r"C:\Windows"))
    }

    fn has_windows_vulkan_loader(system_root: &Path) -> bool {
        system_root.join("System32").join("vulkan-1.dll").is_file()
    }

    /// Generate adaptive Whisper configuration based on hardware
    pub fn get_whisper_config(&self) -> AdaptiveWhisperConfig {
        // Windows-specific override: Always use beam size 2 for stability
        #[cfg(target_os = "windows")]
        {
            return AdaptiveWhisperConfig {
                beam_size: 2,
                temperature: 0.2,
                use_gpu: self.has_gpu_acceleration,
                max_threads: Some(self.cpu_cores.min(8) as usize),
                chunk_size_preference: ChunkSizePreference::Balanced,
            };
        }

        // Platform-adaptive configuration for non-Windows systems
        #[cfg(not(target_os = "windows"))]
        {
            match self.performance_tier {
                PerformanceTier::Ultra => AdaptiveWhisperConfig {
                    beam_size: 5,  // Maximum quality
                    temperature: 0.1,
                    use_gpu: self.has_gpu_acceleration,
                    max_threads: Some(self.cpu_cores.min(8) as usize),
                    chunk_size_preference: ChunkSizePreference::Quality,
                },
                PerformanceTier::High => AdaptiveWhisperConfig {
                    beam_size: 3,  // High quality
                    temperature: 0.2,
                    use_gpu: self.has_gpu_acceleration,
                    max_threads: Some(self.cpu_cores.min(6) as usize),
                    chunk_size_preference: ChunkSizePreference::Balanced,
                },
                PerformanceTier::Medium => AdaptiveWhisperConfig {
                    beam_size: 2,  // Balanced
                    temperature: 0.3,
                    use_gpu: self.has_gpu_acceleration,
                    max_threads: Some(self.cpu_cores.min(4) as usize),
                    chunk_size_preference: ChunkSizePreference::Balanced,
                },
                PerformanceTier::Low => AdaptiveWhisperConfig {
                    beam_size: 1,  // Fast processing
                    temperature: 0.4,
                    use_gpu: false, // Force CPU to avoid GPU overhead on weak hardware
                    max_threads: Some(2),
                    chunk_size_preference: ChunkSizePreference::Fast,
                },
            }
        }
    }

    /// Get recommended chunk duration in milliseconds based on performance tier
    pub fn get_recommended_chunk_duration_ms(&self) -> u32 {
        match self.performance_tier {
            PerformanceTier::Ultra => 25000,   // 25 seconds for maximum accuracy
            PerformanceTier::High => 20000,    // 20 seconds for high quality
            PerformanceTier::Medium => 15000,  // 15 seconds for balance
            PerformanceTier::Low => 10000,     // 10 seconds for responsiveness
        }
    }

    /// Generate comprehensive hardware recommendations for transcription and LLM models
    pub fn get_recommendations(&self) -> HardwareRecommendation {
        let (gpu_name, tier_str) = match (self.gpu_type, self.performance_tier) {
            (GpuType::Metal, PerformanceTier::Ultra) => ("Apple Silicon (Metal GPU)", "Ultra"),
            (GpuType::Metal, _) => ("Apple Silicon (Metal GPU)", "High"),
            (GpuType::Cuda, PerformanceTier::Ultra) => ("NVIDIA CUDA GPU", "Ultra"),
            (GpuType::Cuda, _) => ("NVIDIA CUDA GPU", "High"),
            (GpuType::Vulkan, _) => ("Vulkan GPU", "Medium"),
            (GpuType::OpenCL, _) => ("OpenCL GPU", "Medium"),
            (GpuType::None, PerformanceTier::Medium) => ("CPU High-Performance", "Medium"),
            (GpuType::None, _) => ("CPU Standard", "Low"),
        };

        let (tx_engine, tx_model, llm_provider, llm_model, ctx, explanation) = match self.performance_tier {
            PerformanceTier::Ultra | PerformanceTier::High => (
                "parakeet".to_string(),
                "parakeet-tdt-0.6b-v3-int8".to_string(),
                "builtin-ai".to_string(),
                "qwen3.5:4b".to_string(),
                8192,
                format!(
                    "Hardware de alta performance detectado ({} cores CPU, GPU {}, ~{}GB RAM). Recomendado uso local acelerado por GPU com Parakeet TDT para transcrição instantânea e Qwen 3.5 (4B) para resumos ricos.",
                    self.cpu_cores, gpu_name, self.memory_gb
                ),
            ),
            PerformanceTier::Medium => (
                "parakeet".to_string(),
                "parakeet-tdt-0.6b-v3-int8".to_string(),
                "builtin-ai".to_string(),
                "qwen3.5:4b".to_string(),
                4096,
                format!(
                    "Hardware intermediário detectado ({} cores CPU, {}). Parakeet TDT é recomendado para transcrição leve e Qwen 3.5 local para resumos sem sobrecarregar a memória.",
                    self.cpu_cores, gpu_name
                ),
            ),
            PerformanceTier::Low => (
                "whisper".to_string(),
                "tiny".to_string(),
                "openrouter".to_string(),
                "google/gemini-2.5-flash".to_string(),
                2048,
                format!(
                    "Hardware limitado ({} cores CPU, sem aceleração de GPU dedicada). Recomendado Whisper Tiny local para transcrição de baixo consumo e API externa (OpenRouter/Groq) para resumos sem travar a máquina.",
                    self.cpu_cores
                ),
            ),
        };

        HardwareRecommendation {
            cpu_cores: self.cpu_cores,
            memory_gb: self.memory_gb,
            gpu_type: gpu_name.to_string(),
            has_gpu_acceleration: self.has_gpu_acceleration,
            performance_tier: tier_str.to_string(),
            recommended_transcription_engine: tx_engine,
            recommended_transcription_model: tx_model,
            recommended_summary_provider: llm_provider,
            recommended_summary_model: llm_model,
            max_recommended_context: ctx,
            explanation,
        }
    }
}

/// Tauri command to expose hardware benchmark & recommendation to frontend
#[tauri::command]
pub fn get_hardware_recommendations() -> HardwareRecommendation {
    HardwareProfile::detect().get_recommendations()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_detection() {
        let profile = HardwareProfile::detect();
        assert!(profile.cpu_cores > 0);
        // Performance optimization: remove println! from tests
        log::debug!("Detected profile: {:?}", profile);
    }

    #[test]
    fn test_whisper_config_generation() {
        let profile = HardwareProfile::detect();
        let config = profile.get_whisper_config();

        assert!(config.beam_size >= 1 && config.beam_size <= 5);
        assert!(config.temperature >= 0.0 && config.temperature <= 1.0);

        // Performance optimization: remove println! from tests
        log::debug!("Generated config: {:?}", config);
    }

    #[test]
    fn test_performance_tier_logic() {
        // Test different hardware combinations
        let low_tier = HardwareProfile::calculate_performance_tier(2, &GpuType::None, 4);
        assert_eq!(low_tier, PerformanceTier::Low);

        let high_tier = HardwareProfile::calculate_performance_tier(8, &GpuType::Metal, 16);
        assert_eq!(high_tier, PerformanceTier::Ultra);
    }

    #[test]
    fn hardware_detector_finds_windows_vulkan_loader_in_system32() {
        let temp_dir = tempfile::tempdir().unwrap();
        let system32 = temp_dir.path().join("System32");
        std::fs::create_dir(&system32).unwrap();
        std::fs::write(system32.join("vulkan-1.dll"), []).unwrap();

        assert!(HardwareProfile::has_windows_vulkan_loader(temp_dir.path()));
    }

    #[test]
    fn hardware_detector_rejects_missing_windows_vulkan_loader() {
        let temp_dir = tempfile::tempdir().unwrap();

        assert!(!HardwareProfile::has_windows_vulkan_loader(temp_dir.path()));
    }
}
