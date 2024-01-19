pushd $HOME/edk2
source ./edksetup.sh
patch -N -r - ./Conf/target.txt < /workspace/.devcontainer/patch/target.txt.patch
popd

source $HOME/osbook/devenv/buildenv.sh