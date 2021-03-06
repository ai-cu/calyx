/* verilator lint_off WIDTH */
module std_div #(
    parameter width = 32
) (
    input                    clk,
    input                    go,
    input        [width-1:0] left,
    input        [width-1:0] right,
    output logic [width-1:0] out_remainder,
    output logic [width-1:0] out_quotient,
    output logic             done
);

  logic [width-1:0] dividend;
  logic [(width-1)*2:0] divisor;
  logic [width-1:0] quotient;
  logic [width-1:0] quotient_msk;
  logic start, running, finished;

  assign start = go && !running;
  assign finished = !quotient_msk && running;

  always_latch @(posedge clk) begin
    if (start && left == 0) begin
      out_remainder <= 0;
      out_quotient <= 0;
    end
    else if (finished) begin
      out_remainder <= dividend;
      out_quotient <= quotient;
    end
  end

  always_ff @(posedge clk) begin
    if (start && left == 0)
      done <= 1;
    else if (finished)
      done <= 1;
    else
      done <= 0;
  end

  always_latch @(posedge clk) begin
    if (!go)
      running <= 0;
    else if (start)
      running <= 1;
    else if (finished)
      running <= 0;
  end

  always_latch @(posedge clk) begin
    if (start) begin
      dividend <= left;
      divisor <= right << width - 1;
      quotient <= 0;
      quotient_msk <= 1 << width - 1;
    end else begin
      if (divisor <= dividend) begin
        dividend <= dividend - divisor;
        quotient <= quotient | quotient_msk;
      end
      divisor <= divisor >> 1;
      quotient_msk <= quotient_msk >> 1;
    end
  end

  `ifdef VERILATOR
    // Simulation self test against unsynthesizable implementation.
    always @(posedge clk) begin
      if (finished && dividend != $unsigned(((left % right) + right) % right))
        $error(
          "\nstd_mod_pipe: (Remainder) Computed and golden outputs do not match!\n",
          "left: %0d", $unsigned(left),
          "  right: %0d\n", $unsigned(right),
          "expected: %0d", $unsigned(((left % right) + right) % right),
          "  computed: %0d", $unsigned(out_remainder)
        );

      if (finished && quotient != $unsigned(left / right))
        $error(
          "\nstd_mod_pipe: (Quotient) Computed and golden outputs do not match!\n",
          "left: %0d", $unsigned(left),
          "  right: %0d\n", $unsigned(right),
          "expected: %0d", $unsigned(left / right),
          "  computed: %0d", $unsigned(out_quotient)
        );
    end
  `endif
endmodule

module std_mult_pipe #(
    parameter width = 32
) (
    input  logic [width-1:0] left,
    input  logic [width-1:0] right,
    input  logic             go,
    input  logic             clk,
    output logic [width-1:0] out,
    output logic             done
);
  logic [width-1:0] rtmp;
  logic [width-1:0] ltmp;
  logic [width-1:0] out_tmp;
  reg done_buf[1:0];
  always_ff @(posedge clk) begin
    if (go) begin
      rtmp <= right;
      ltmp <= left;
      out_tmp <= rtmp * ltmp;
      out <= out_tmp;

      done <= done_buf[1];
      done_buf[0] <= 1'b1;
      done_buf[1] <= done_buf[0];
    end else begin
      rtmp <= 0;
      ltmp <= 0;
      out_tmp <= 0;
      out <= 0;

      done <= 0;
      done_buf[0] <= 0;
      done_buf[1] <= 0;
    end
  end
endmodule

// ===============Signed operations that wrap unsigned ones ===============
/* verilator lint_off WIDTH */
module std_sdiv #(
    parameter width = 32
) (
    input                     clk,
    input                     go,
    input  signed [width-1:0] left,
    input  signed [width-1:0] right,
    output logic  [width-1:0] out_remainder,
    output logic  [width-1:0] out_quotient,
    output logic              done
);

  logic signed [width-1:0] left_abs;
  logic signed [width-1:0] right_abs;
  logic signed [width-1:0] comp_out_remainder;
  logic signed [width-1:0] comp_out_quotient;

  assign right_abs = right[width-1] == 1 ? -right : right;
  assign left_abs = left[width-1] == 1 ? -left : left;
  assign out_quotient =
    (left[width-1] == 1) ^ (right[width-1] == 1) ? -comp_out_quotient : comp_out_quotient;
  assign out_remainder = left[width-1] == 1 ? $signed(right - comp_out_remainder) : comp_out_remainder;

  std_div #(
    .width(width)
  ) comp (
    .clk(clk),
    .done(done),
    .go(go),
    .left(left_abs),
    .right(right_abs),
    .out_quotient(comp_out_quotient),
    .out_remainder(comp_out_remainder)
  );

  `ifdef VERILATOR
    // Simulation self test against unsynthesizable implementation.
    always @(posedge clk) begin
      if (done && out_quotient != $signed(left / right))
        $error(
          "\nstd_sdiv: (Quotient) Computed and golden outputs do not match!\n",
          "left: %0d", left,
          "  right: %0d\n", right,
          "expected: %0d", $signed(left / right),
          "  computed: %0d", $signed(out_quotient)
        );
      if (done && out_remainder != $signed(((left % right) + right) % right))
        $error(
          "\nstd_sdiv: (Remainder) Computed and golden outputs do not match!\n",
          "left: %0d", left,
          "  right: %0d\n", right,
          "expected: %0d", $signed(((left % right) + right) % right),
          "  computed: %0d", $signed(out_remainder)
        );
    end
  `endif
endmodule


//==================== Unsynthesizable primitives =========================
module std_mult #(
    parameter width = 32
) (
    input  logic [width-1:0] left,
    input  logic [width-1:0] right,
    output logic [width-1:0] out
);
  assign out = left * right;
endmodule

module std_mod #(
    parameter width = 32
) (
    input  logic [width-1:0] left,
    input  logic [width-1:0] right,
    output logic [width-1:0] out
);
  assign out = left % right;
endmodule
