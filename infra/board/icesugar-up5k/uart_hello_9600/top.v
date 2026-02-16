module top #(
    parameter integer CLK_HZ = 12_000_000,
    parameter integer BAUD = 9_600,
    parameter integer LED_TOGGLE_HZ = 2
) (
    input  wire clk,
    output wire LED_R,
    output wire LED_G,
    output wire LED_B,
    output wire TX
);
    localparam integer BAUD_DIV = CLK_HZ / BAUD;
    localparam integer LED_HALF_PERIOD = CLK_HZ / LED_TOGGLE_HZ;
    localparam integer MSG_LEN = 13;
    localparam integer GAP_BAUD_TICKS = BAUD;

    reg [31:0] baud_cnt = 32'd0;
    reg baud_tick = 1'b0;

    reg [31:0] led_cnt = 32'd0;
    reg led_green_on = 1'b0;

    reg [9:0] tx_shift = 10'h3FF;
    reg [3:0] tx_bit_idx = 4'd0;
    reg tx_active = 1'b0;
    reg tx_out = 1'b1;

    reg [4:0] msg_idx = 5'd0;
    reg [15:0] gap_ticks = 16'd0;

    function [7:0] msg_byte;
        input [4:0] idx;
        begin
            case (idx)
                5'd0:  msg_byte = 8'h48; // H
                5'd1:  msg_byte = 8'h45; // E
                5'd2:  msg_byte = 8'h4C; // L
                5'd3:  msg_byte = 8'h4C; // L
                5'd4:  msg_byte = 8'h4F; // O
                5'd5:  msg_byte = 8'h20; // space
                5'd6:  msg_byte = 8'h57; // W
                5'd7:  msg_byte = 8'h4F; // O
                5'd8:  msg_byte = 8'h52; // R
                5'd9:  msg_byte = 8'h4C; // L
                5'd10: msg_byte = 8'h44; // D
                5'd11: msg_byte = 8'h0D; // \r
                5'd12: msg_byte = 8'h0A; // \n
                default: msg_byte = 8'h3F; // ?
            endcase
        end
    endfunction

    always @(posedge clk) begin
        // UART baud tick generation.
        if (baud_cnt == BAUD_DIV - 1) begin
            baud_cnt <= 32'd0;
            baud_tick <= 1'b1;
        end else begin
            baud_cnt <= baud_cnt + 32'd1;
            baud_tick <= 1'b0;
        end

        // Green LED heartbeat (0.5s toggle -> 1s full period).
        if (led_cnt == LED_HALF_PERIOD - 1) begin
            led_cnt <= 32'd0;
            led_green_on <= ~led_green_on;
        end else begin
            led_cnt <= led_cnt + 32'd1;
        end

        if (baud_tick) begin
            if (tx_active) begin
                tx_out <= tx_shift[0];
                tx_shift <= {1'b1, tx_shift[9:1]};

                if (tx_bit_idx == 4'd9) begin
                    tx_bit_idx <= 4'd0;
                    tx_active <= 1'b0;
                    tx_out <= 1'b1;

                    if (msg_idx == MSG_LEN - 1) begin
                        msg_idx <= 5'd0;
                        gap_ticks <= GAP_BAUD_TICKS[15:0];
                    end else begin
                        msg_idx <= msg_idx + 5'd1;
                    end
                end else begin
                    tx_bit_idx <= tx_bit_idx + 4'd1;
                end
            end else if (gap_ticks != 0) begin
                gap_ticks <= gap_ticks - 16'd1;
                tx_out <= 1'b1;
            end else begin
                tx_shift <= {1'b1, msg_byte(msg_idx), 1'b0};
                tx_active <= 1'b1;
                tx_bit_idx <= 4'd0;
                tx_out <= 1'b1;
            end
        end
    end

    // iCESugar RGB LED is active-low.
    assign LED_G = ~led_green_on;
    assign LED_R = 1'b1;
    assign LED_B = 1'b1;

    assign TX = tx_out;
endmodule
