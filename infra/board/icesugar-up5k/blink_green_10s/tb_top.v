`timescale 1ns/1ps

module tb_top;
    localparam integer CLK_HZ = 10;
    localparam integer TOGGLE_SEC = 3;
    localparam integer EXPECTED_INTERVAL = CLK_HZ * TOGGLE_SEC;

    reg clk = 1'b0;
    wire led_r;
    wire led_g;
    wire led_b;

    integer cycle = 0;
    integer toggle_count = 0;
    integer last_toggle_cycle = -1;
    integer fail = 0;
    reg prev_led_g;

    top #(
        .CLK_HZ(CLK_HZ),
        .TOGGLE_SEC(TOGGLE_SEC)
    ) dut (
        .clk(clk),
        .LED_R(led_r),
        .LED_G(led_g),
        .LED_B(led_b)
    );

    always #1 clk = ~clk;

    initial begin
        prev_led_g = led_g;

        while ((toggle_count < 3) && (cycle < 300)) begin
            @(posedge clk);
            cycle = cycle + 1;

            if (led_g !== prev_led_g) begin
                if (last_toggle_cycle != -1) begin
                    if ((cycle - last_toggle_cycle) != EXPECTED_INTERVAL) begin
                        $display("FAIL: interval=%0d cycles, expected=%0d", cycle - last_toggle_cycle, EXPECTED_INTERVAL);
                        fail = 1;
                    end
                end
                $display("toggle #%0d at cycle %0d, LED_G=%0d", toggle_count + 1, cycle, led_g);
                last_toggle_cycle = cycle;
                toggle_count = toggle_count + 1;
                prev_led_g = led_g;
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
