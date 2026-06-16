#!/usr/bin/env python3
"""Build a minimal valid RPM v4 package without external dependencies."""
import struct, gzip, io, os

release_dir = "/home/tyin/agent-circle/target/release"
service_path = "/home/tyin/agent-circle/packaging/agent-circle.service"
output_path = "/home/tyin/agent-circle/target/rpm/agent-circle-0.1.0-1.x86_64.rpm"
os.makedirs(os.path.dirname(output_path), exist_ok=True)

def make_cpio(files):
    buf = io.BytesIO()
    for src, dst in files:
        st = os.stat(src)
        name = "." + "/" + dst
        namesize = len(name) + 1
        namesize_padded = (namesize + 3) & ~3
        hdr = f"070701{st.st_ino&0xffffffff:08x}{st.st_mode&0xffff:08x}{st.st_uid:08x}{st.st_gid:08x}{st.st_nlink:08x}{int(st.st_mtime):08x}{st.st_size:08x}000000000000000000000000{namesize:08x}00000000"
        buf.write(hdr.encode())
        buf.write(name.encode() + b'\x00')
        pad = namesize_padded - namesize
        if pad: buf.write(b'\x00' * pad)
        with open(src, 'rb') as f: data = f.read()
        buf.write(data)
        dpad = (4 - (len(data) % 4)) % 4
        if dpad: buf.write(b'\x00' * dpad)
    buf.write(b"0707010000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000b00000000TRAILER!!!\x00")
    return buf.getvalue()

def make_header(tags_data):
    entries, data_store = [], b""
    for tag, rtype, value in tags_data:
        if rtype in (6, 8): val = str(value).encode() + b'\x00'
        elif rtype == 4: val = struct.pack(">i", value)
        elif rtype == 3: val = struct.pack(">h", value)
        elif rtype == 7: val = value
        else: val = value
        offset = len(data_store)
        data_store += val
        entries.append(struct.pack(">iiii", tag, rtype, offset, 1))
    return struct.pack(">IIII", 0x8eade801, 0, len(entries)//16, len(data_store)) + b"".join(entries) + data_store

files = [
    (service_path.replace("agent-circle/", ""), "usr/lib/systemd/user/agent-circle.service"),
    (release_dir + "/agent-circle", "usr/bin/agent-circle"),
]
files = [(release_dir + "/agent-circle", "usr/bin/agent-circle"),
         ("/home/tyin/agent-circle/packaging/agent-circle.service", "usr/lib/systemd/user/agent-circle.service")]

cpio_data = make_cpio(files)
gzbuf = io.BytesIO()
with gzip.GzipFile(fileobj=gzbuf, mode='wb') as gz: gz.write(cpio_data)
payload = gzbuf.getvalue()

main_tags = [
    (1000, 6, "agent-circle"), (1001, 6, "0.1.0"), (1002, 6, "1"),
    (1004, 8, "P2P social infrastructure for AI agents"),
    (1005, 8, "Agent Circle is a decentralized P2P social protocol."),
    (1014, 6, "MIT"), (1020, 6, "https://github.com/TYEclipse/agent-circle"),
    (1021, 6, "linux"), (1022, 6, "x86_64"),
]
main_header = make_header(main_tags)
sig_tags = [(1000, 4, len(payload) + len(main_header))]
sig_header = make_header(sig_tags)

name = b"agent-circle".ljust(66, b'\x00')
lead = struct.pack(">BBH", 0xed, 0xab, 0xeedb) + b'\x01\x00' + name + b'\x00' * 16

full = lead + sig_header + struct.pack(">I", len(payload) + len(main_header))[4:] + main_header + payload
with open(output_path, 'wb') as f: f.write(full)

print(f"OK: {output_path} ({os.path.getsize(output_path)} bytes)")
