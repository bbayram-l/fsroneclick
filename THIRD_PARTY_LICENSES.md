# Third-Party Notices and Credits

OptiOneClick is an open-source automation and deployment utility. This tool automates and coordinates the configuration of several exceptional open-source libraries, runtimes, and community projects.

The credit for the underlying technologies belongs entirely to their original authors:

---

## Inspiration & Foundation

### [DLSS5oneclick](https://github.com/faisalkindi/DLSS5oneclick)
- **Author**: [Faisal Al-Kindi (faisalkindi)](https://github.com/faisalkindi)
- **Role**: The original conceptual inspiration, user experience model, and foundational architectural design for creating an automated one-click enhancement manager. OptiOneClick adapts this vision to create an automated, hardware-aware deployment workflow for AMD Radeon GPUs and universal FSR/OptiScaler setups.
- **License**: MIT License

---

## Core Runtimes & Add-ons

### [OptiScaler](https://github.com/optiscaler/OptiScaler)
- **Authors**: [cdozdil](https://github.com/cdozdil) and the OptiScaler Development Team
- **Role**: Core translation layer intercepting DLSS (NVNGX), XeSS, and native engine calls, redirecting them into AMD FSR 3.1, AMD FSR 4.1.1 Redstone, and Frame Generation.
- **License**: GNU General Public License v3.0 / MIT components
- **Repository**: https://github.com/optiscaler/OptiScaler

### [AMD FidelityFX™ SDK & FSR "Redstone"](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK)
- **Author**: Advanced Micro Devices, Inc. (AMD) / GPUOpen
- **Role**: High-performance temporal upscaling (FSR 3.1, FSR 4.1.1), frame generation, Contrast Adaptive Sharpening (CAS), and RDNA 3 WMMA INT8 tensor acceleration.
- **License**: MIT License
- **Copyright**: (c) 2026 Advanced Micro Devices, Inc.

### [ReShade](https://reshade.me) & [reshade-shaders](https://github.com/crosire/reshade-shaders)
- **Author**: [crosire](https://github.com/crosire)
- **Role**: Generic post-processing injector framework that hosts offline sharpening and color grading shaders.
- **License**: BSD 3-Clause License
- **Copyright**: (c) 2014 Patrick Mours

### [RenoDX](https://github.com/clshortfuse/renodx)
- **Authors**: [clshortfuse](https://github.com/clshortfuse) and the RenoDX Community
- **Role**: Native HDR reconstruction, tonemapping, and color space transform add-on for ReShade.
- **License**: MIT License
- **Repository**: https://github.com/clshortfuse/renodx

### [OptiPatcher](https://github.com/optiscaler/OptiScaler/wiki)
- **Authors**: OptiPatcher and community contributors
- **Role**: In-memory ASI patcher neutralizing proprietary RTX hardware signature checks in games that crash when DLSS Frame Generation is requested on non-RTX cards.
- **License**: Community Open Source

### [LumeniteFX](https://github.com/umar-afzaal/LumeniteFX)
- **Author**: [Afzaal (Kaidō)](https://github.com/umar-afzaal)
- **Role**: Advanced motion-vector reconstruction, tonemapping, and post-processing shaders.
- **License**: MIT License

---

## Libraries & UI Framework

### [egui / eframe](https://github.com/emilk/egui)
- **Author**: [Emil Ernerfeldt (emilk)](https://github.com/emilk)
- **Role**: Immediate mode GUI framework for native desktop rendering.
- **License**: MIT License / Apache License 2.0

### Typography & Fonts
- **[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono)**: Created by JetBrains — SIL Open Font License 1.1
