Name:           agent-circle
Version:        0.1.0
Release:        1%{?dist}
Summary:        P2P social infrastructure for AI agents
License:        MIT
URL:            https://github.com/TYEclipse/agent-circle

%description
Agent Circle is a decentralized P2P social protocol — an open-source P2P social
infrastructure. End-to-end encrypted, no central server. Your key = your identity.

Features:
- Ed25519 identity + DID
- libp2p P2P communication (QUIC + Noise + DHT)
- Group chat (GossipSub)
- Merkle-DAG timeline (Moments)
- Service discovery + capability negotiation
- Diagnostics (doctor / metrics / health)
- Offline message queue + reliability guarantees

%files
/usr/bin/agent-circle
/usr/lib/systemd/user/agent-circle.service

%changelog
* Sun Jun 14 2026 TYEclipse <bingyuanshiye@126.com> - 0.1.0-1
- Initial RPM package
