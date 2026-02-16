`include "uart_rx.v"
`include "uart_tx.v"

module top (
    input  wire clk,
    input  wire RX,
    output wire TX,
    output wire LED_R,
    output wire LED_G,
    output wire LED_B
);
    localparam integer CLK_HZ = 12_000_000;
    localparam integer BAUD = 115_200;

    wire       rx_ready;
    wire [7:0] rx_data;
    wire       rx_idle;
    wire       rx_eop;

    reg        tx_start = 1'b0;
    reg  [7:0] tx_data = 8'h00;
    wire       tx_busy;

    reg [7:0] req[0:7];
    reg [3:0] req_len = 4'd0;

    reg [7:0] resp[0:5];
    reg [2:0] tx_idx = 3'd0;
    reg       tx_pending = 1'b0;
    reg [23:0] heartbeat = 24'd0;

    function [15:0] crc16_byte;
        input [15:0] crc_in;
        input [7:0] data;
        integer i;
        reg [15:0] crc;
        begin
            crc = crc_in ^ data;
            for (i = 0; i < 8; i = i + 1) begin
                if (crc[0])
                    crc = (crc >> 1) ^ 16'hA001;
                else
                    crc = crc >> 1;
            end
            crc16_byte = crc;
        end
    endfunction

    wire [15:0] req_crc_calc = crc16_byte(
                                crc16_byte(
                                crc16_byte(
                                crc16_byte(
                                crc16_byte(
                                crc16_byte(16'hFFFF, req[0]), req[1]), req[2]), req[3]), req[4]), req[5]);

    wire [15:0] resp_crc_calc = crc16_byte(
                                 crc16_byte(
                                 crc16_byte(
                                 crc16_byte(16'hFFFF, req[0]), 8'h01), 8'h01), 8'h01);

    uart_rx #(CLK_HZ, BAUD) u_rx (
        .clk(clk),
        .rx(RX),
        .rx_ready(rx_ready),
        .rx_data(rx_data),
        .rx_idle(rx_idle),
        .rx_eop(rx_eop)
    );

    uart_tx #(CLK_HZ, BAUD) u_tx (
        .clk(clk),
        .tx_start(tx_start),
        .tx_data(tx_data),
        .tx(TX),
        .tx_busy(tx_busy)
    );

    always @(posedge clk) begin
        tx_start <= 1'b0;
        heartbeat <= heartbeat + 24'd1;

        if (rx_ready) begin
            if (req_len < 4'd8) begin
                req[req_len] <= rx_data;
                req_len <= req_len + 4'd1;
            end else begin
                req[0] <= rx_data;
                req_len <= 4'd1;
            end
        end

        if (rx_eop) begin
            // Handle FC01: read coil 0, qty 1; answer with coil=1.
            if (req_len == 4'd8 &&
                req[1] == 8'h01 &&
                req[2] == 8'h00 && req[3] == 8'h00 &&
                req[4] == 8'h00 && req[5] == 8'h01 &&
                req[6] == req_crc_calc[7:0] && req[7] == req_crc_calc[15:8]) begin

                resp[0] <= req[0];
                resp[1] <= 8'h01;
                resp[2] <= 8'h01;
                resp[3] <= 8'h01;
                resp[4] <= resp_crc_calc[7:0];
                resp[5] <= resp_crc_calc[15:8];
                tx_idx <= 3'd0;
                tx_pending <= 1'b1;
            end
            req_len <= 4'd0;
        end

        if (tx_pending && !tx_busy) begin
            tx_data <= resp[tx_idx];
            tx_start <= 1'b1;
            if (tx_idx == 3'd5) begin
                tx_idx <= 3'd0;
                tx_pending <= 1'b0;
            end else begin
                tx_idx <= tx_idx + 3'd1;
            end
        end
    end

    // Active-low RGB LED outputs.
    assign LED_R = 1'b1;
    assign LED_G = ~(tx_pending | heartbeat[23]);
    assign LED_B = ~(rx_idle);
endmodule
