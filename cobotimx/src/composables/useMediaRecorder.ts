import { ref, onUnmounted, type Ref } from "vue";

export type RecordingMode = "audio" | "video";

export type RecorderState =
  | "idle"
  | "requesting"
  | "recording"
  | "stopped"
  | "denied";

export interface UseMediaRecorderReturn {
  state: Ref<RecorderState>;
  durationSec: Ref<number>;
  stream: Ref<MediaStream | null>;
  error: Ref<string | null>;

  start(): Promise<void>;
  stop(): Promise<File | null>;
  cancel(): void;
  getBlobUrl(): string | null;
  requestPreviewStream(): Promise<MediaStream | null>;
}

function getPreferredMimeType(mode: RecordingMode): string {
  if (mode === "audio") {
    const candidates = [
      "audio/webm;codecs=opus",
      "audio/webm",
      "audio/ogg;codecs=opus",
    ];
    for (const m of candidates) {
      if (MediaRecorder.isTypeSupported(m)) return m;
    }
  } else {
    const candidates = ["video/webm;codecs=vp8,opus", "video/webm;codecs=vp9,opus", "video/webm"];
    for (const m of candidates) {
      if (MediaRecorder.isTypeSupported(m)) return m;
    }
  }
  return "";
}

function getFileExtension(mimeType: string): string {
  if (mimeType.includes("ogg")) return "ogg";
  if (mimeType.includes("mp4")) return "mp4";
  return "webm";
}

function formatTimestamp(): string {
  const now = new Date();
  const y = now.getFullYear();
  const mo = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  const h = String(now.getHours()).padStart(2, "0");
  const mi = String(now.getMinutes()).padStart(2, "0");
  const s = String(now.getSeconds()).padStart(2, "0");
  return `${y}${mo}${d}_${h}${mi}${s}`;
}

// ── WAV encoder (PCM 16-bit mono) ──────────────────────────────────────
// Converts Float32Array samples (range -1..1) to a WAV Blob.
// WAV is the simplest audio format — 44-byte header + raw PCM samples.
// It requires NO codec, so it plays in every browser and media player.
function encodeWav(samples: Float32Array[], sampleRate: number): Blob {
  const totalSamples = samples.reduce((sum, s) => sum + s.length, 0);
  const dataSize = totalSamples * 2; // 16-bit = 2 bytes per sample
  const buffer = new ArrayBuffer(44 + dataSize);
  const view = new DataView(buffer);

  function wstr(offset: number, s: string) {
    for (let i = 0; i < s.length; i++) {
      view.setUint8(offset + i, s.charCodeAt(i));
    }
  }

  // RIFF chunk
  wstr(0, "RIFF");
  view.setUint32(4, 36 + dataSize, true); // file size - 8
  wstr(8, "WAVE");

  // fmt chunk
  wstr(12, "fmt ");
  view.setUint32(16, 16, true); // subchunk size (PCM = 16)
  view.setUint16(20, 1, true);  // audio format (1 = PCM)
  view.setUint16(22, 1, true);  // mono
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true); // byte rate
  view.setUint16(32, 2, true);  // block align
  view.setUint16(34, 16, true); // bits per sample

  // data chunk
  wstr(36, "data");
  view.setUint32(40, dataSize, true);

  // Write PCM samples
  let offset = 44;
  for (const chunk of samples) {
    for (let i = 0; i < chunk.length; i++) {
      const s = Math.max(-1, Math.min(1, chunk[i]));
      view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7fff, true);
      offset += 2;
    }
  }

  return new Blob([buffer], { type: "audio/wav" });
}

export function useMediaRecorder(mode: Ref<RecordingMode> | RecordingMode): UseMediaRecorderReturn {
  const modeRef = typeof mode === "string" ? ref(mode) : mode;

  const state = ref<RecorderState>("idle");
  const durationSec = ref(0);
  const stream = ref<MediaStream | null>(null);
  const error = ref<string | null>(null);

  // ── MediaRecorder state (video mode) ──
  let mediaRecorder: MediaRecorder | null = null;
  let chunks: Blob[] = [];

  // ── PCM capture state (audio mode) ──
  let audioContext: AudioContext | null = null;
  let pcmSamples: Float32Array[] = [];
  let pcmSampleRate = 0;

  let durationTimer: ReturnType<typeof setInterval> | null = null;
  let blobUrl: string | null = null;

  const isAudio = modeRef.value === "audio";

  function startDurationTimer() {
    durationSec.value = 0;
    durationTimer = setInterval(() => {
      durationSec.value += 0.1;
    }, 100);
  }

  function stopDurationTimer() {
    if (durationTimer) {
      clearInterval(durationTimer);
      durationTimer = null;
    }
  }

  function isSecureContext(): boolean {
    return window.isSecureContext || location.hostname === "localhost" || location.hostname === "127.0.0.1";
  }

  async function start(): Promise<void> {
    if (state.value === "recording" || state.value === "requesting") return;

    if (!isSecureContext()) {
      error.value = "录音功能需要 HTTPS 连接";
      state.value = "denied";
      return;
    }

    state.value = "requesting";
    error.value = null;

    try {
      const mediaStream = await navigator.mediaDevices.getUserMedia(
        isAudio
          ? {
              audio: {
                sampleRate: { ideal: 48000 },
                echoCancellation: false,
                noiseSuppression: false,
                autoGainControl: false,
              },
            }
          : { video: { facingMode: "user", width: { ideal: 1280 }, height: { ideal: 720 } }, audio: true }
      );
      stream.value = mediaStream;

      if (isAudio) {
        // ── Audio: PCM capture via Web Audio API ──
        // This avoids Chrome's buggy MediaRecorder for audio-only streams.
        // AudioWorklet-based PCM capture — more reliable than the deprecated
        // ScriptProcessorNode. Does NOT need connection to destination, so
        // Chrome won't suspend it mid-recording, and there's no feedback.
        pcmSamples = [];
        // Force 48000 Hz to match the getUserMedia constraint — mismatched
        // sample rates can cause Chrome to emit "Failed to reconcile encoded
        // audio times" and produce empty/corrupted capture data.
        audioContext = new AudioContext({ sampleRate: 48000 });
        pcmSampleRate = audioContext.sampleRate;

        if (audioContext.state === "suspended") {
          await audioContext.resume();
        }

        // Inline the AudioWorklet processor as a Blob URL
        const workletCode = `
          class PcmCaptureProcessor extends AudioWorkletProcessor {
            process(inputs) {
              const input = inputs[0];
              if (input && input.length > 0) {
                // Always take channel 0 — if the mic is stereo, we just
                // use the left channel rather than relying on Chrome's
                // downmix (which can be buggy on some drivers).
                const src = input[0];
                if (src && src.length > 0) {
                  const copy = new Float32Array(src.length);
                  copy.set(src);
                  this.port.postMessage(copy.buffer, [copy.buffer]);
                }
              }
              return true;
            }
          }
          registerProcessor('pcm-capture', PcmCaptureProcessor);
        `;
        const blob = new Blob([workletCode], { type: "application/javascript" });
        const workletUrl = URL.createObjectURL(blob);
        await audioContext.audioWorklet.addModule(workletUrl);
        URL.revokeObjectURL(workletUrl);

        const workletNode = new AudioWorkletNode(audioContext, "pcm-capture");
        workletNode.port.onmessage = (e: MessageEvent) => {
          const buf = e.data as ArrayBuffer;
          pcmSamples.push(new Float32Array(buf));
        };

        const onStateChange = () => {
          if (audioContext?.state === "suspended" && state.value === "recording") {
            audioContext.resume().catch(() => {});
          }
        };
        audioContext.addEventListener("statechange", onStateChange);

        const source = audioContext.createMediaStreamSource(mediaStream);
        source.connect(workletNode);

        // Chrome MAY stop processing an audio graph that has no output
        // connected. Route through a zero-gain node to destination so
        // Chrome sees a "consumer" but no actual sound is produced.
        const silenceNode = audioContext.createGain();
        silenceNode.gain.value = 0;
        workletNode.connect(silenceNode);
        silenceNode.connect(audioContext.destination);
      } else {
        // ── Video: MediaRecorder (works fine for video) ──
        chunks = [];
        const mimeType = getPreferredMimeType("video");
        mediaRecorder = new MediaRecorder(mediaStream, mimeType ? { mimeType } : undefined);

        mediaRecorder.ondataavailable = (e: BlobEvent) => {
          if (e.data.size > 0) chunks.push(e.data);
        };

        mediaRecorder.onerror = () => {
          error.value = "录制过程出错";
          cleanupMedia();
          state.value = "idle";
        };

        mediaRecorder.start(1000);
      }

      startDurationTimer();
      state.value = "recording";
    } catch (err: unknown) {
      const domErr = err as DOMException;
      if (domErr.name === "NotAllowedError") {
        const device = isAudio ? "麦克风" : "摄像头";
        error.value = `无法访问${device}，请在浏览器设置中允许${device}权限`;
      } else if (domErr.name === "NotFoundError") {
        const device = isAudio ? "麦克风" : "摄像头";
        error.value = `未检测到${device}设备`;
      } else {
        error.value = `录制启动失败: ${domErr.message || String(err)}`;
      }
      state.value = "denied";
    }
  }

  async function stop(): Promise<File | null> {
    if (state.value !== "recording") return null;

    const dur = Math.round(durationSec.value);

    if (isAudio) {
      // ── Audio: disconnect PCM capture and encode WAV ──
      stopDurationTimer();

      // Disconnect audio graph
      if (audioContext) {
        audioContext.close().catch(() => {});
        audioContext = null;
      }

      if (dur < 1 || pcmSamples.length === 0) {
        pcmSamples = [];
        cleanupMedia();
        state.value = "idle";
        return null;
      }

      const wavBlob = encodeWav(pcmSamples, pcmSampleRate || 48000);
      const filename = `voice_${formatTimestamp()}.wav`;
      const file = new File([wavBlob], filename, { type: "audio/wav" });

      // Create preview blob url
      if (blobUrl) URL.revokeObjectURL(blobUrl);
      blobUrl = URL.createObjectURL(wavBlob);

      pcmSamples = [];
      cleanupMedia();
      state.value = "stopped";
      return file;
    }

    // ── Video: MediaRecorder stop ──
    if (!mediaRecorder) return null;
    const rec = mediaRecorder; // capture in local to satisfy TS narrowing

    return new Promise((resolve) => {
      rec.onstop = () => {
        stopDurationTimer();

        if (dur < 1 || chunks.length === 0) {
          chunks = [];
          cleanupMedia();
          state.value = "idle";
          resolve(null);
          return;
        }

        const mimeType = rec.mimeType || "video/webm";
        const ext = getFileExtension(mimeType);
        const filename = `video_${formatTimestamp()}.${ext}`;
        const file = new File(chunks, filename, { type: mimeType });

        const combinedBlob = new Blob(chunks, { type: mimeType });
        if (blobUrl) URL.revokeObjectURL(blobUrl);
        blobUrl = URL.createObjectURL(combinedBlob);

        chunks = [];
        cleanupMedia();
        state.value = "stopped";
        resolve(file);
      };

      rec.stop();
    });
  }

  function cancel() {
    stopDurationTimer();

    if (isAudio) {
      // Disconnect PCM capture and discard samples
      if (audioContext) {
        audioContext.close().catch(() => {});
        audioContext = null;
      }
      pcmSamples = [];
    } else if (mediaRecorder && state.value === "recording") {
      try {
        mediaRecorder.stop();
      } catch {
        // already stopped
      }
      chunks = [];
    }

    cleanupMedia();
    state.value = "idle";
  }

  function getBlobUrl(): string | null {
    return blobUrl;
  }

  async function requestPreviewStream(): Promise<MediaStream | null> {
    if (!isSecureContext()) {
      error.value = "摄像头功能需要 HTTPS 连接";
      state.value = "denied";
      return null;
    }

    try {
      const mediaStream = await navigator.mediaDevices.getUserMedia({
        video: {
          facingMode: "user",
          width: { ideal: 1280 },
          height: { ideal: 720 },
        },
        audio: true,
      });
      stream.value = mediaStream;
      return mediaStream;
    } catch (err: unknown) {
      const domErr = err as DOMException;
      if (domErr.name === "NotAllowedError") {
        error.value = "无法访问摄像头，请在浏览器设置中允许摄像头权限";
      } else if (domErr.name === "NotFoundError") {
        error.value = "未检测到摄像头设备";
      } else {
        error.value = `摄像头启动失败: ${domErr.message || String(err)}`;
      }
      state.value = "denied";
      return null;
    }
  }

  function cleanupMedia() {
    if (stream.value) {
      stream.value.getTracks().forEach((t) => t.stop());
      stream.value = null;
    }
    mediaRecorder = null;
  }

  onUnmounted(() => {
    stopDurationTimer();
    if (audioContext) {
      audioContext.close().catch(() => {});
      audioContext = null;
    }
    cleanupMedia();
    if (blobUrl) {
      URL.revokeObjectURL(blobUrl);
      blobUrl = null;
    }
  });

  return {
    state,
    durationSec,
    stream,
    error,
    start,
    stop,
    cancel,
    getBlobUrl,
    requestPreviewStream,
  };
}
