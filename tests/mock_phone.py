#!/usr/bin/env python3
"""Camera2-protocol fixture. Synthetic video; never a phone performance result."""
import argparse
import hashlib
import hmac
import json
import secrets
import socket
import ssl
import struct
import subprocess
import tempfile
import threading
import time
from pathlib import Path


class MockPhone:
    def __init__(self, password=None, uncompressed=False, drop_after=None):
        self.password = password
        self.uncompressed = uncompressed
        self.drop_after = drop_after
        self.dropped_at = None
        self.reconfigured_after_drop = None
        self.configurations = 0
        self.directory = tempfile.TemporaryDirectory(prefix="opencam-fixture-")
        folder = Path(self.directory.name)
        self.cert, self.key = folder / "cert.pem", folder / "key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", str(self.key),
                        "-out", str(self.cert), "-days", "1", "-subj", "/CN=OpenCam fixture"], check=True, capture_output=True)
        der = ssl.PEM_cert_to_DER_cert(self.cert.read_text())
        self.pin = hashlib.sha256(der).hexdigest()
        self.token = secrets.token_hex(16)
        self.server = socket.socket()
        self.server.bind(("127.0.0.1", 0))
        self.server.listen()
        self.server.settimeout(0.5)
        self.port = self.server.getsockname()[1]
        self.tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.tls.load_cert_chain(self.cert, self.key)
        self.running = True
        self.thread = threading.Thread(target=self.accept, daemon=True)
        self.thread.start()

    @property
    def link(self):
        return f"opencam://127.0.0.1:{self.port}?token={self.token}&pin={self.pin}"

    def capabilities(self):
        camera = {"id": "fixture", "label": "Synthetic test camera", "facing": 1,
                  "sizes": [[640, 360], [960, 720], [1280, 720]], "fpsRanges": [[15, 30], [60, 60]],
                  "iso": [50, 6400], "exposureNs": [100000, 100000000], "zoom": [1, 10],
                  "ev": [-6, 6], "noiseModes":[0,1,2], "edgeModes":[0,1,2], "aberrationModes":[0,1,2], "lensCorrectionModes":[0,1,2], "manualSensor": True, "manualFocus": True, "focusMax": 10,
                  "awbModes": [0, 1, 2, 5, 6], "oisModes": [0, 1], "stabilizationModes": [0, 1],
                  "flash": True, "aeLock": True, "awbLock": True, "raw": False,
                  "characteristics": {"fixture": True}}
        codecs = [{"name": "fixture.avc", "label": "H.264", "mime": "video/avc", "bitrate": [100000, 50000000]},
                  {"name": "fixture.hevc", "label": "HEVC", "mime": "video/hevc", "bitrate": [100000, 50000000]}]
        if self.uncompressed:
            camera.update(yuvSizes=camera["sizes"], rgbaSizes=camera["sizes"], raw=True, rawSizes=[[640,360]])
            codecs += [{"name":"opencam.i420","label":"YUV420 · uncompressed","mime":"video/x-opencam-i420"},
                       {"name":"opencam.rgba","label":"RGBA · uncompressed","mime":"video/x-opencam-rgba"}]
        return {"type": "capabilities", "protocol": 1, "device": "Protocol fixture · synthetic video", "cameras": [camera], "codecs": codecs}

    @staticmethod
    def read_exact(connection, length):
        data = bytearray()
        while len(data) < length:
            chunk = connection.recv(length - len(data))
            if not chunk:
                raise EOFError()
            data.extend(chunk)
        return bytes(data)

    @staticmethod
    def units(settings):
        w, h, fps = settings["width"], settings["height"], settings["fps"]
        if settings["codec"] in ("opencam.i420", "opencam.rgba"):
            pixel_format = "rgba" if settings["codec"] == "opencam.rgba" else "yuv420p"
            frame = subprocess.run(["ffmpeg","-hide_banner","-loglevel","error","-f","lavfi","-i",f"testsrc2=size={w}x{h}","-frames:v","1","-threads","1","-pix_fmt",pixel_format,"-f","rawvideo","pipe:1"],check=True,capture_output=True,timeout=15).stdout
            assert len(frame) == w*h*(4 if pixel_format=="rgba" else 1.5)
            return [frame]
        hevc = "hevc" in settings["codec"]
        command = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", f"testsrc2=size={w}x{h}:rate={fps}",
                   "-frames:v", str(fps * 2), "-c:v", "libx265" if hevc else "libx264", "-preset", "ultrafast"]
        if hevc:
            command += ["-x265-params", f"aud=1:keyint={fps}:bframes=0:repeat-headers=1:scenecut=0:pools=1:frame-threads=1:log-level=error"]
        else:
            command += ["-tune", "zerolatency", "-x264-params", f"aud=1:keyint={fps}:repeat-headers=1:scenecut=0:threads=1"]
        # Synthetic fixture effects exercise live controls; they are not a phone/HAL model.
        zoom = min(10., max(1., float(settings.get("zoom", 1))))
        brightness = .18 if settings.get("torch") else 0.
        brightness += float(settings.get("ev", 0)) * .025
        if settings.get("manual"):
            brightness += .08 * __import__("math").log2(max(1., float(settings.get("iso", 100))) / 100.)
        filters = [f"crop=trunc(iw/{zoom}/2)*2:trunc(ih/{zoom}/2)*2", f"scale={w}:{h}", f"eq=brightness={max(-.9,min(.9,brightness))}"]
        # Fixture effects are FFmpeg approximations; Android tests exercise the real GLES shaders.
        sx, sy = max(1.,float(settings.get("stretchX",1))), max(1.,float(settings.get("stretchY",1)))
        if sx != 1 or sy != 1: filters += [f"crop=trunc(iw/{sx}/2)*2:trunc(ih/{sy}/2)*2",f"scale={w}:{h}"]
        if settings.get("distortion"): filters += [f"lenscorrection=k1={float(settings['distortion'])*.5}"]
        rotation = settings.get("phoneRotation",0)
        if rotation in (90,270): filters += ["transpose="+str(1 if rotation==90 else 2)]; w,h=h,w
        elif rotation==180: filters += ["hflip","vflip"]
        if settings.get("phoneMirror"): filters += ["hflip"]
        ow,oh=settings.get("outputWidth",0) or w,settings.get("outputHeight",0) or h
        if (ow,oh)!=(w,h):
            mode=settings.get("outputMode",0)
            if mode==2: filters += [f"scale={ow}:{oh}"]
            elif mode==1: filters += [f"scale={ow}:{oh}:force_original_aspect_ratio=increase",f"crop={ow}:{oh}"]
            else: filters += [f"scale={ow}:{oh}:force_original_aspect_ratio=decrease:force_divisible_by=2",f"pad={ow}:{oh}:(ow-iw)/2:(oh-ih)/2:black"]
        command += ["-vf", ",".join(filters)]
        command += ["-f", "hevc" if hevc else "h264", "pipe:1"]
        stream = subprocess.run(command, check=True, capture_output=True, timeout=30).stdout
        delimiter = b"\x00\x00\x00\x01\x46\x01" if hevc else b"\x00\x00\x00\x01\x09"
        parts = stream.split(delimiter)
        units = [delimiter + part for part in parts[1:]]
        assert len(units) == fps * 2, f"Expected access-unit boundaries, got {len(units)}"
        return units

    def accept(self):
        while self.running:
            try:
                raw, _ = self.server.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            threading.Thread(target=self.peer, args=(raw,), daemon=True).start()

    def peer(self, raw):
        connection = None
        epoch = [0]
        lock = threading.Lock()
        settings = {}

        def send(kind, body, generation=None):
            with lock:
                if generation is not None and epoch[0] != generation:
                    return False
                connection.sendall(struct.pack(">BI", kind, len(body)) + body)
                return True

        def event(value, generation=None):
            return send(1, json.dumps(value).encode(), generation)

        def video(config, generation):
            try:
                frames = self.units(config)
                if epoch[0] != generation:
                    return
                uncompressed = config["codec"] in ("opencam.i420", "opencam.rgba")
                event({"type": "configured", "width": config.get("outputWidth",0) or (config["height"] if config.get("phoneRotation",0)%180 else config["width"]), "height": config.get("outputHeight",0) or (config["width"] if config.get("phoneRotation",0)%180 else config["height"]), "phoneProcessed":True,
                       "fps": config["fps"], "mime": ("video/x-opencam-rgba" if config["codec"]=="opencam.rgba" else "video/x-opencam-i420") if uncompressed else ("video/hevc" if "hevc" in config["codec"] else "video/avc"), "timestampsRealtime": True, "settings": config}, generation)
                start = time.monotonic()
                index = 0
                applied = tuple(config.get(k) for k in ("zoom","torch","ev","manual","iso","stretchX","stretchY","distortion","bulge"))
                while epoch[0] == generation and self.running:
                    effect = tuple(settings.get(k) for k in ("zoom","torch","ev","manual","iso","stretchX","stretchY","distortion","bulge"))
                    if effect != applied:
                        config.update(settings)
                        frames = self.units(config)
                        applied = effect
                        index = 0
                        start = time.monotonic()
                    pts = int(time.monotonic() * 1e6)
                    flags = 1 if index % config["fps"] == 0 else 0
                    if self.drop_after is not None and self.dropped_at is None and time.monotonic()-start >= self.drop_after:
                        self.dropped_at = time.monotonic()
                        connection.shutdown(socket.SHUT_RDWR)
                        break
                    if uncompressed:
                        frame = frames[index % len(frames)]
                        for offset in range(0,len(frame),65536):
                            if not send(3,struct.pack(">QIII",pts,len(frame),offset,0x10c10000)+frame[offset:offset+65536],generation): break
                    elif not send(2, struct.pack(">QI", pts, flags) + frames[index % len(frames)], generation):
                        break
                    index += 1
                    time.sleep(max(0, start + index / config["fps"] - time.monotonic()))
            except (OSError, ssl.SSLError, subprocess.SubprocessError):
                return

        try:
            raw.settimeout(5)
            connection = self.tls.wrap_socket(raw, server_side=True)
            connection.settimeout(15)
            authenticated = False
            challenge = None
            salt = secrets.token_bytes(16)
            while self.running:
                kind, length = struct.unpack(">BI", self.read_exact(connection, 5))
                if kind != 1 or not 0 < length <= 65536:
                    break
                command = json.loads(self.read_exact(connection, length))
                operation = command["type"]
                if not authenticated:
                    if challenge is None:
                        if operation != "hello" or not secrets.compare_digest(command.get("token", ""), self.token): break
                        if self.password:
                            challenge = secrets.token_bytes(32)
                            event({"type":"auth","salt":salt.hex(),"challenge":challenge.hex(),"iterations":210000})
                            continue
                    else:
                        key = hashlib.pbkdf2_hmac("sha256",self.password.encode(),salt,210000)
                        proof = hmac.new(key,b"opencam-auth-v1\0"+challenge+bytes.fromhex(self.pin),hashlib.sha256).hexdigest()
                        if operation != "auth" or not secrets.compare_digest(command.get("proof",""),proof):
                            event({"type":"error","message":"Password incorrect"}); break
                    authenticated = True
                    event(self.capabilities())
                elif operation == "ping":
                    event({"type": "pong", "sent": command["sent"], "phoneUs": int(time.monotonic() * 1e6)})
                elif operation == "configure":
                    with lock:
                        epoch[0] += 1
                    self.configurations += 1
                    if self.dropped_at is not None and self.reconfigured_after_drop is None:
                        self.reconfigured_after_drop = time.monotonic()-self.dropped_at
                    settings = command["settings"].copy()
                    threading.Thread(target=video, args=(settings.copy(), epoch[0]), daemon=True).start()
                elif operation == "controls":
                    settings.update(command["settings"])
                    event({"type": "controls", "settings": settings})
                    event({"type":"metadata","values":{"android.control.zoomRatio":settings.get("zoom",1),
                        "android.sensor.sensitivity":settings.get("iso",100),"android.sensor.exposureTime":settings.get("exposureNs",16666667)}})
                elif operation == "raw" and self.uncompressed:
                    data = b"II*\0" + bytes(range(256))*401
                    transfer = secrets.token_bytes(16)
                    event({"type":"raw_file_begin","id":transfer.hex(),"bytes":len(data),"format":"dng"})
                    for offset in range(0,len(data),65536): send(4,transfer+struct.pack(">Q",offset)+data[offset:offset+65536])
                    event({"type":"raw_file_end","id":transfer.hex(),"sha256":hashlib.sha256(data).hexdigest()})
                elif operation == "stop":
                    epoch[0] += 1
                    event({"type": "stopped"})
        except (OSError, EOFError, ssl.SSLError):
            pass
        finally:
            epoch[0] += 1
            if connection:
                connection.close()
            raw.close()

    def close(self):
        self.running = False
        self.server.close()
        self.thread.join(timeout=2)
        self.directory.cleanup()


def smoke(binary):
    binary = Path(binary).resolve(strict=True)
    phone = MockPhone()
    try:
        invalid = phone.link.replace(phone.pin, "00" * 32)
        denied = subprocess.run([str(binary), "--pair", invalid, "--seconds", "1"], capture_output=True, text=True, timeout=15)
        assert denied.returncode != 0 and "certificate" in denied.stderr.lower(), denied.stderr
        invalid_token = phone.link.replace(phone.token, "00" * 16)
        denied = subprocess.run([str(binary), "--pair", invalid_token, "--seconds", "1"], capture_output=True, text=True, timeout=15)
        assert denied.returncode != 0, "Wrong pairing token accepted"
        result = subprocess.run([str(binary), "--pair", phone.link, "--seconds", "3"], capture_output=True, text=True, timeout=30)
        assert result.returncode == 0, result.stderr
        report = json.loads(result.stdout)
        assert report["decodedFrames"] >= 60, report
        result = subprocess.run([str(binary), "--pair", phone.link, "--benchmark"], capture_output=True, text=True, timeout=60)
        assert result.returncode == 0, result.stderr
        benchmark = json.loads(result.stdout)
        assert len(benchmark["results"]) == 2, benchmark
        assert all(r["decoded_fps"] >= 25 for r in benchmark["results"]), benchmark
        assert benchmark["winner"] is not None, benchmark
        formats = []
        for extra in [["--output","720x720"], ["--size","960x720","--fps","27","--output","640x480","--crop"],
                      ["--size","960x720","--fps","27","--output","720x720","--process-on","phone"],
                      ["--size","960x720","--fps","27","--output","640x480","--process-on","desktop","--backend","cpu","--stretch","1.4"]]:
            run = subprocess.run([str(binary),"--pair",phone.link,"--seconds","2"]+extra,capture_output=True,text=True,timeout=30)
            assert run.returncode == 0, run.stderr
            frame = json.loads(run.stdout)
            dimensions = [720,720] if "720x720" in extra else [640,480]
            assert frame["lastFrame"] is not None and frame["lastFrame"][:2] == dimensions and frame["decodedFrames"] >= 40, {"arguments":extra,"report":frame,"stderr":run.stderr}
            if "--process-on" in extra:
                if "desktop" in extra:
                    assert frame["processing"]["mode"] == "Desktop" and frame["processing"]["outputFps"] > 0, frame
                else:
                    assert frame["configured"]["width"] == 720 and frame["configured"]["height"] == 720, frame
            formats.append(frame)
        protected = MockPhone("fixture-password")
        try:
            for password in [None,"wrong-password"]:
                command = [str(binary),"--pair",protected.link,"--seconds","1"]
                if password: command += ["--password-stdin"]
                denied = subprocess.run(command,input=(password+"\n") if password else None,capture_output=True,text=True,timeout=15)
                assert denied.returncode != 0 and "password" in denied.stderr.lower(), denied.stderr
                assert "cameras" not in denied.stdout and "decodedFrames" not in denied.stdout
            permitted = subprocess.run([str(binary),"--pair",protected.link,"--seconds","2","--password-stdin"],input="fixture-password\n",capture_output=True,text=True,timeout=30)
            assert permitted.returncode == 0 and json.loads(permitted.stdout)["decodedFrames"] >= 40, permitted.stderr
        finally: protected.close()
        pixels = MockPhone(uncompressed=True,drop_after=.3)
        uncompressed = []
        try:
            for codec in ("opencam.i420","opencam.rgba"):
                run=subprocess.run([str(binary),"--pair",pixels.link,"--codec",codec,"--size","640x360","--fps","27","--seconds","2","--raw"],cwd=pixels.directory.name,capture_output=True,text=True,timeout=20)
                assert run.returncode==0,run.stderr
                result=json.loads(run.stdout)
                assert result["configured"]["settings"]["codec"]==codec and result["decodedFrames"]>=40 and result["lastFrame"][:2]==[640,360],result
                saved=Path(pixels.directory.name)/result["rawFile"]
                assert saved.read_bytes()==b"II*\0"+bytes(range(256))*401
                saved.unlink()
                uncompressed.append(result)
            run=subprocess.run([str(binary),"--pair",pixels.link,"--benchmark","--size","640x360","--fps","27"],capture_output=True,text=True,timeout=60)
            assert run.returncode==0,run.stderr
            all_formats=json.loads(run.stdout)
            assert len(all_formats["results"])==4 and all(r["decoded_fps"]>=24 for r in all_formats["results"]) and all_formats["winner"] is not None,all_formats
            assert pixels.configurations>=3 and pixels.reconfigured_after_drop is not None and pixels.reconfigured_after_drop<2,pixels.reconfigured_after_drop
        finally: pixels.close()
        print(json.dumps({"uncompressed":uncompressed,"allTransportFormatsBenchmark":all_formats,"authenticatedReconnectSeconds":pixels.reconfigured_after_drop,"rawFileIntegrity":True,"test": "synthetic protocol/decoder integration", "stream": report, "benchmark": benchmark, "formats": formats, "passwordGate": "missing/wrong denied before capabilities; correct accepted"}, indent=2))
    finally:
        phone.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--probe", type=Path)
    options = parser.parse_args()
    if options.probe:
        smoke(options.probe)
    else:
        phone = MockPhone()
        print(phone.link, flush=True)
        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            phone.close()
