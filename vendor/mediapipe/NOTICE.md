MediaPipe Tasks Vision 0.10.35 and Google Selfie Segmentation landscape model, Apache-2.0.

Model source: https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmenter_landscape/float16/1/selfie_segmenter_landscape.tflite
SHA-256: 490e9ea734313e0de10fa0cd9e3c6133e36ea4db2b7a49bde9ef019f72796b8e
Model card: https://storage.googleapis.com/mediapipe-assets/Model%20Card%20MediaPipe%20Selfie%20Segmentation.pdf
Source/API: https://github.com/google-ai-edge/mediapipe

Model and code are unchanged. The model recognizes prominent people, not arbitrary subjects; hair, fingers, low light and fast motion can produce imperfect edges. Inference runs on the phone, with no cloud service.
# Desktop ONNX conversion

The desktop additionally embeds the Apache-2.0 ONNX Community conversion of
MediaPipe Selfie Segmentation Landscape, revision
`2497d5bec26c626c7b3c4edc6e1fefc21b64f6c3`:
https://huggingface.co/onnx-community/mediapipe_selfie_segmentation_landscape

Source: https://huggingface.co/onnx-community/mediapipe_selfie_segmentation_landscape/resolve/2497d5bec26c626c7b3c4edc6e1fefc21b64f6c3/onnx/model.onnx
SHA-256: `7a0adcfdb1715d3b0ff0f61486d8a181c33f4a208343a5abfccbc6540e872d24`.
Input RGB floats 0–1, 256×144. Same person-segmentation limitations as the phone model.
Bundled ONNX Runtime C libraries retain the notices extracted alongside them by
`tools/fetch_onnx_runtime.py`; the Rust bindings retain their upstream licenses.
