wget https://registrationcenter-download.intel.com/akdlm/IRC_NAS/7deeaac4-f605-4bcf-a81b-ea7531577c61/l_BaseKit_p_2023.1.0.46401_offline.sh
sudo sh ./l_BaseKit_p_2023.1.0.46401_offline.sh

wget https://registrationcenter-download.intel.com/akdlm/IRC_NAS/1ff1b38a-8218-4c53-9956-f0b264de35a4/l_HPCKit_p_2023.1.0.46346_offline.sh
sudo sh ./l_HPCKit_p_2023.1.0.46346_offline.sh

# Setup Intel OneAPI environment initialization
echo 'source /opt/intel/oneapi/setvars.sh > /dev/null' >> ~/.bashrc
source ~/.bashrc
