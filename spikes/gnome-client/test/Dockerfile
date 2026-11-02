ARG BASE=ferry-gnome46
FROM ${BASE}
USER root
RUN apt-get update && apt-get install -y --no-install-recommends libwebkit2gtk-4.1-0 gir1.2-gtk-4.0 && rm -rf /var/lib/apt/lists/*
USER dev
