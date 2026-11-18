"""Independent byte encoding of the published canonical-hash golden vectors."""
import hashlib
import struct

cases = [
    (
        "default", "Tên riêng", "# Tên trong thân\nĐoạn sau.",
        "f735c19566fdff2931d7a5944bb57882c4a915bd0fecb29f1d3c46119ed2c79b",
    ),
    (
        "markdown", "Tiếng mưa",
        "# Tie\u0302\u0301ng mưa\r\n\r\n\r\n  \r\n\u0110oa\u0323n hai\t\U0001f9d1\u200d\U0001f33e\n",
        "8ba939c87570af370209535a832b9b86d550eaf5a5f59f3c1f135647c8061769",
    ),
]
for fmt, title, source, expected in cases:
    encoded = b"vink-document-content"
    lengths = []
    for field in (fmt, title, source):
        raw = field.encode("utf-8")
        lengths.append(len(raw))
        encoded += struct.pack(">Q", len(raw)) + raw
    actual = hashlib.sha256(encoded).hexdigest()
    assert actual == expected, (fmt, expected, actual)
    print(f"PASS {fmt}: byte lengths={lengths}; sha256={actual}")
print("2 independent literal-byte vectors PASS; this does not execute a Markdown parser.")
