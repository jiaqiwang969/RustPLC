module top #(
    parameter integer CLK_HZ = 12_000_000,
    parameter integer TOGGLE_SEC = 10
) (
    input  wire clk,
    output wire LED_R,
    output wire LED_G,
    output wire LED_B
);
    localparam integer MAX = CLK_HZ * TOGGLE_SEC;
    localparam integer WIDTH = $clog2(MAX);

    reg [WIDTH-1:0] counter = {WIDTH{1'b0}};
    reg led_on = 1'b0;

    always @(posedge clk) begin
        if (counter == MAX - 1) begin
            counter <= {WIDTH{1'b0}};
            led_on <= ~led_on;
        end else begin
            counter <= counter + 1'b1;
        end
    end

    // iCESugar RGB LED pins are active-low.
    assign LED_G = ~led_on;
    assign LED_R = 1'b1;
    assign LED_B = 1'b1;
endmodule
