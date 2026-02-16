`timescale 1ns/1ps

module tb_blink_10s;
    localparam integer CLK_HZ = 10;
    localparam integer TOGGLE_SEC = 3;
    localparam integer EXPECTED_INTERVAL = CLK_HZ * TOGGLE_SEC;

    reg clk = 1'b0;
    wire led_g_o;

    integer cycle = 0;
    integer last_toggle_cycle = -1;
    integer toggle_count = 0;
    integer fail = 0;
    integer prev_valid = 0;
    reg prev_led;

    blink_10s #(
        .CLK_HZ(CLK_HZ),
        .TOGGLE_SEC(TOGGLE_SEC)
    ) dut (
        .clk_i(clk),
        .led_g_o(led_g_o)
    );

    always #1 clk = ~clk;

    initial begin
        prev_led = 1'b0;
        prev_valid = 0;
        // Run long enough to observe several toggles.
        while ((toggle_count < 3) && (cycle < 300)) begin
            @(posedge clk);
            cycle = cycle + 1;

            if ((led_g_o === 1'b0) || (led_g_o === 1'b1)) begin
                if (prev_valid && (led_g_o !== prev_led)) begin
                    if (last_toggle_cycle != -1) begin
                        if ((cycle - last_toggle_cycle) != EXPECTED_INTERVAL) begin
                            $display("FAIL: interval=%0d cycles, expected=%0d", cycle - last_toggle_cycle, EXPECTED_INTERVAL);
                            fail = 1;
                        end
                    end
                    $display("toggle #%0d at cycle %0d, led_g_o=%0d", toggle_count + 1, cycle, led_g_o);
                    last_toggle_cycle = cycle;
                    toggle_count = toggle_count + 1;
                end
                prev_led = led_g_o;
                prev_valid = 1;
            end
        end

        if (toggle_count < 3) begin
            $display("FAIL: not enough toggles observed (%0d)", toggle_count);
            fail = 1;
        end

        if (fail) begin
            $display("SIM_RESULT: FAIL");
            $fatal(1);
        end else begin
            $display("SIM_RESULT: PASS");
            $finish;
        end
    end
endmodule
