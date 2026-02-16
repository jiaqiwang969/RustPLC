module blink_10s #(
    // iCESugar-pro uses 25 MHz clock on P6.
    parameter integer CLK_HZ = 25_000_000,
    // Toggle every 10 seconds.
    parameter integer TOGGLE_SEC = 10
) (
    input      clk_i,
    output reg led_g_o
);
localparam integer MAX = CLK_HZ * TOGGLE_SEC;
localparam integer WIDTH = $clog2(MAX);

wire rst_s;
wire clk_s;

assign clk_s = clk_i;
rst_gen rst_inst (.clk_i(clk_s), .rst_i(1'b0), .rst_o(rst_s));

reg  [WIDTH-1:0] cpt_s;
wire [WIDTH-1:0] cpt_next_s = cpt_s + 1'b1;
wire             end_s = cpt_s == MAX - 1;

always @(posedge clk_s) begin
    cpt_s <= (rst_s || end_s) ? {WIDTH{1'b0}} : cpt_next_s;

    if (rst_s)
        led_g_o <= 1'b0;
    else if (end_s)
        led_g_o <= ~led_g_o;
end
endmodule
