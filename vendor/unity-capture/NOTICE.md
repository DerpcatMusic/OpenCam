Unity Capture shared-memory transport by Bernhard Schelling, MIT, based on UnityCam by MHD Yamen Saraiji.
https://github.com/schellingb/UnityCapture
Copied from pyvirtualcam native_windows_unity_capture/shared_memory; only the separately MIT-licensed transport is used. OpenCam adds UnmapViewOfFile cleanup on reconnect.
The camera driver itself is installed separately. Native sender selects the first 64-bit device and uses a 500 ms timeout to stop showing frames after sender exit.
