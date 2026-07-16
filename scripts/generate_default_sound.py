import array
import math
import pathlib
import wave

SAMPLE_RATE = 48_000
OUTPUT = pathlib.Path(__file__).resolve().parents[1] / "sounds" / "default-notification.wav"


def silence(duration: float) -> list[float]:
    return [0.0] * round(SAMPLE_RATE * duration)


def bell_tone(frequency: float, duration: float) -> list[float]:
    frame_count = round(SAMPLE_RATE * duration)
    samples: list[float] = []
    for index in range(frame_count):
        time = index / SAMPLE_RATE
        attack = min(1.0, time / 0.018)
        release = min(1.0, max(0.0, (duration - time) / 0.22))
        decay = math.exp(-1.15 * time)
        envelope = attack * release * decay
        fundamental = math.sin(2.0 * math.pi * frequency * time)
        second = 0.34 * math.sin(2.0 * math.pi * frequency * 2.01 * time)
        third = 0.13 * math.sin(2.0 * math.pi * frequency * 3.02 * time)
        samples.append(envelope * (fundamental + second + third))
    return samples


signal = (
    silence(0.04)
    + bell_tone(659.25, 0.56)
    + silence(0.08)
    + bell_tone(783.99, 0.78)
    + silence(0.14)
)
peak = max(abs(sample) for sample in signal)
gain = 0.88 / peak
pcm = array.array("h", (round(max(-1.0, min(1.0, sample * gain)) * 32767) for sample in signal))

OUTPUT.parent.mkdir(parents=True, exist_ok=True)
with wave.open(str(OUTPUT), "wb") as audio:
    audio.setnchannels(1)
    audio.setsampwidth(2)
    audio.setframerate(SAMPLE_RATE)
    audio.writeframes(pcm.tobytes())

print(OUTPUT)
