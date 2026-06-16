FROM python:3.12-slim

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY bell_simulator.py .

RUN chmod +x bell_simulator.py

ENV BACKEND_URL=http://backend:8080
ENV SIMULATOR_MODE=hourly
ENV SIMULATOR_INTERVAL=60
ENV SIMULATOR_ERROR_RATE=0.05

ENTRYPOINT ["python", "-u", "bell_simulator.py"]
CMD ["--mode", "daemon"]
