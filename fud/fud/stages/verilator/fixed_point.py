import numpy as np


def binary_to_base10(bitstring: str) -> int:
    """Takes a binary number in string form
    e.g. "1010" and returns the
    corresponding base 10 number.
    """
    out = 0
    for bit in bitstring:
        out = (out << 1) | int(bit)
    return out


def fp_to_decimal(value, width, int_width, is_signed):
    """Takes in a fixed point number with the
    bit width, integer bit width, and signedness, and
    returns the decimal value."""
    frac_width = width - int_width
    begin_index = 1 if is_signed else 0

    int_bits = value[begin_index:int_width]
    frac_bits = value[int_width:width]
    integer_value = int(int_bits, 2)
    fractional_value = float(
        int(frac_bits, 2) / (2 ** frac_width)
    )
    fp_value = float(integer_value + fractional_value)
    if is_signed and value[0] == '1':
        # If the sign bit is high,
        # return the negated value.
        return fp_value * -1

    return fp_value


def decimal_to_fp(num, width, int_width, is_signed):
    """Given the number, width, integer bit and fractional bit,
    returns the fixed point representation.

    If the fraction cannot be represented exactly in
    fixed point, then it raises an exception.

    This is done in two steps:
    1. Produce the binary representation of the
       fixed point number with the given `width`,
       `int_width`.
    2. Convert this binary representation to base 10.
    """
    # Separate into integer and fractional parts.
    float_value = float(num * -1 if (is_signed and num < 0) else num)
    integer_part, fractional_part = str(float_value).split(".")

    if is_signed:
        prefix = '1' if num < 0 else '0'
        no_signed_bit_width = int_width - 1
    else:
        prefix = ''
        no_signed_bit_width = int_width

    int_bits = prefix + np.binary_repr(
        int(integer_part),
        width=no_signed_bit_width
    )

    # Multiply fractional part with 2 ** frac_bit to turn into integer.
    frac_width = width - int_width
    fractional_repr = float("0." + fractional_part) * float(2 ** frac_width)
    frac_bits = np.binary_repr(
        int(fractional_repr),
        width=frac_width
    )

    _, fractional_excess = str(fractional_repr).split(".")
    # TODO(cgyurgyik): Eventually, we want to use
    # truncation for values that cannot be exactly
    # represented.
    if fractional_excess != "0":
        # Verify we can represent the number in fixed point.
        raise Exception(
            f"{num} cannot be represented as the "
            f"type: {'' if is_signed else 'u'}fix<{width}, {int_width}>"
        )
    elif len(int_bits) > int_width:
        raise Exception(
            f"{int_bits} causes overflow, provide a larger integer width."
        )
    elif len(frac_bits) > frac_width:
        raise Exception(
            f"{frac_bits} causes overflow, provide a larger fractional width."
        )
    else:
        # Given the binary form of the integer part and fractional part of
        # the decimal, simply append the two strings and convert to base 10.
        return binary_to_base10(int_bits + frac_bits)