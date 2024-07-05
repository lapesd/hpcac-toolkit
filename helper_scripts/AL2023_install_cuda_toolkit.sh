sudo dnf install -y dkms kernel-devel kernel-modules-extra
sudo dnf config-manager --add-repo https://developer.download.nvidia.com/compute/cuda/repos/amzn2023/x86_64/cuda-amzn2023.repo
sudo dnf clean expire-cache
sudo dnf -y module install nvidia-driver:latest-dkms
sudo dnf install -y cuda-toolkit

# REBOOT
# Check installation: https://repost.aws/articles/ARwfQMxiC-QMOgWykD9mco1w/how-do-i-install-nvidia-gpu-driver-cuda-toolkit-and-optionally-nvidia-container-toolkit-in-amazon-linux-2023-al2023
