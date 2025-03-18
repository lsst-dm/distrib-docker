#! /bin/bash
set -x
gcsfuse -o allow_other --implicit-dirs eups-gc-storage-dev $MNT_DIR
nginx -g "daemon off;"
