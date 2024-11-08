import matplotlib.pyplot as plt
from itertools import groupby
import numpy as np
import scipy.stats as st
import math
import re


class Packet:
    def __init__(self, mode, ptype, src, dest, size, txsize, seq, CW, cwsize, dispatch, time):
        self.mode = mode
        self.ptype = ptype
        self.src = src
        self.dest = dest
        self.size = int(size)
        self.txsize = int(txsize)
        self.seq = int(seq)
        self.CW = int(CW)
        self.cwsize = int(cwsize)
        self.dispatch = float(dispatch)
        self.time = float(time)


def parse_packet(line):
    pattern = r"s\[(R|T),([DAR|C]{1,3}),([0-9A-Fa-f]+)->([0-9A-Fa-f]+),(\d+)\((\d+)\),(\d+),(\d+),(\d+),(\d+),?([\d.]*)\]"
    match = re.match(pattern, line.strip())

    if match:
        mode, ptype, src, dest, size, txsize, seq, CW, cwsize, dispatch, time = match.groups()
        return Packet(mode, ptype, src, dest, size, txsize, seq, CW, cwsize, dispatch, time or None)
    return None


def read_packets(filename):
    packets = []
    with open(filename, 'r') as file:
        for line in file:
            packet = parse_packet(line)
            if packet:
                packets.append(packet)
    return packets


def calculate_throughput(packets):
    packets = [packet for packet in packets if packet.mode ==
               'R' and packet.ptype == 'D']
    total_size = sum(packet.size for packet in packets)
    return total_size / 60


def seq(packet):
    return packet.seq


def calculate_delay(packets):
    data = [list([p.time for p in packet if not (math.isnan(p.time))])
            for key, packet in groupby(packets, seq)]
    times = [max(timings) - min(timings)
             for timings in data]
    if times:
        mean = np.mean(times)
        print(f'Mean: {mean}')
        std = np.std(times, mean=mean)
        print(f'Std: {std}')
        conf = st.norm.interval(0.95, loc=mean)
        print(f'Conf: {conf}')
        return mean
    return 0


def plot_throughput(distances, throughputs, payload_size):
    plt.plot(distances, throughputs, marker='o',
             label=f'Throughput for {payload_size} byte(s) payload')
    plt.title(f"Throughput vs Distance for Payload {payload_size} bytes")
    plt.xlabel("Distance (m)")
    plt.ylabel("Throughput (Bytes per second)")
    plt.grid(True)
    plt.legend()
    plt.show()


def plot_delays(distances, delays, payload_size):
    plt.plot(distances, delays, marker='o',
             label=f'Delay for {payload_size} byte(s) payload')
    plt.title(f"Delay vs Distance for Payload {payload_size} bytes")
    plt.xlabel("Distance (m)")
    plt.ylabel("Delay (ms)")
    plt.grid(True)
    plt.legend()
    plt.show()


def main():
    distances = [0, 10, 20, 30, 40, 50, 60]
    payload_sizes = [1, 100, 180]

    for payload in payload_sizes:
        throughputs = []

        for distance in distances:
            filename = f"data/{distance}cm-{payload}b.rx.dat"
            packets = read_packets(filename)

            throughput = calculate_throughput(packets)
            throughputs.append(throughput)

        plot_throughput(distances, throughputs, payload)

        delays = []

        for distance in distances:
            filename = f"data/{distance}cm-{payload}b.tx.dat"
            packets = read_packets(filename)

            print(filename)
            delay = calculate_delay(packets)
            delays.append(delay)

        plot_delays(distances, delays, payload)


if __name__ == "__main__":
    main()
