#! /bin/bash
set -x
#gcsfuse -o allow_other --implicit-dirs eups-gc-storage-dev /eups
nginx -g "daemon off;"
