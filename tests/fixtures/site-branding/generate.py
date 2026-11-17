"""Generate deterministic synthetic PNG/APNG fixtures using the standard library."""
from pathlib import Path
import struct
import zlib


def chunk(kind, data):
    return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data) & 0xffffffff)


root = Path(__file__).parent
header = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!IIBBBBB', 2, 3, 8, 6, 0, 0, 0))
pixels = zlib.compress((b'\0' + b'\x10\x80\x70\xff' * 2) * 3)
root.joinpath('logo.png').write_bytes(header + chunk(b'IDAT', pixels) + chunk(b'IEND', b''))
frame = lambda seq: struct.pack('!IIIIIHHBB', seq, 2, 3, 0, 0, 1, 10, 0, 0)
root.joinpath('animated.png').write_bytes(header + chunk(b'acTL', struct.pack('!II', 2, 0)) + chunk(b'fcTL', frame(0)) + chunk(b'IDAT', pixels) + chunk(b'fcTL', frame(1)) + chunk(b'fdAT', struct.pack('!I', 2) + pixels) + chunk(b'IEND', b''))
