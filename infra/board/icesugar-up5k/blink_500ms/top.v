module top(
    input  wire clk,
    output wire LED_R,
    output wire LED_G,
    output wire LED_B
);
    // iCESugar board uses a 12 MHz input clock from iCELink.
    localparam integer CLK_HZ = 12_000_000;
    localparam integer BLINK_PERIOD_MS = 10_000;
    // Toggle every 5 s => full on+off cycle is 10 s.
    localparam integer TOGGLE_CYCLES = (CLK_HZ / 1000 * BLINK_PERIOD_MS) / 2;

    // 60,000,000 cycles at 12 MHz needs at least 26 bits.
    reg [25:0] div_counter = 26'd0;
    reg led_on = 1'b0;

    always @(posedge clk) begin
        if (div_counter == TOGGLE_CYCLES - 1) begin
            div_counter <= 26'd0;
            led_on <= ~led_on;
        end else begin
            div_counter <= div_counter + 1'b1;
        end
    end

    // LED pins are active-low on iCESugar.
    assign LED_R = ~led_on;
    assign LED_G = ~led_on;
    assign LED_B = ~led_on;
endmodule
