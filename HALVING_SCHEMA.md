Steps to Configure the Game

Calculate the Number of Slots in One Week

The game operates on a slot-based system (similar to blockchain networks like Solana). Assuming an average of 2.5 slots per second:

Seconds in a week: <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mn>7</mn><mo>×</mo><mn>24</mn><mo>×</mo><mn>60</mn><mo>×</mo><mn>60</mn><mo>=</mo><mn>604</mn><mo separator="true">,</mo><mn>800</mn></mrow><annotation encoding="application/x-tex"> 7 \times 24 \times 60 \times 60 = 604,800 </annotation></semantics></math> seconds.
Slots in a week: <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mn>604</mn><mo separator="true">,</mo><mn>800</mn><mo>×</mo><mn>2.5</mn><mo>=</mo><mn>1</mn><mo separator="true">,</mo><mn>512</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> 604,800 \times 2.5 = 1,512,000 </annotation></semantics></math> slots.


So, the total number of slots in a week (<math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><msub><mi>S</mi><mtext>week</mtext></msub></mrow><annotation encoding="application/x-tex"> S_{\text{week}} </annotation></semantics></math>) is approximately 1,512,000 slots.


Set the Halving Interval

Bitcoin has 32 halvings over its lifetime. To have the same number of halvings occur within one week, divide the total slots in a week by 32:
<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><semantics><mrow><mi>H</mi><mo>=</mo><mfrac><msub><mi>S</mi><mtext>week</mtext></msub><mn>32</mn></mfrac><mo>=</mo><mfrac><mrow><mn>1</mn><mo separator="true">,</mo><mn>512</mn><mo separator="true">,</mo><mn>000</mn></mrow><mn>32</mn></mfrac><mo>=</mo><mn>47</mn><mo separator="true">,</mo><mn>250</mn><mtext> slots</mtext></mrow><annotation encoding="application/x-tex">H = \frac{S_{\text{week}}}{32} = \frac{1,512,000}{32} = 47,250 \text{ slots}</annotation></semantics></math>

Set the halving interval (<math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>H</mi></mrow><annotation encoding="application/x-tex"> H </annotation></semantics></math>) to 47,250 slots. This means the reward rate will halve every 47,250 slots (approximately every 5.25 hours).


Determine the Initial Reward Rate

The total supply (<math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi></mrow><annotation encoding="application/x-tex"> T </annotation></semantics></math>) will be minted over time, approaching a maximum of <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mn>2</mn><mo>×</mo><mi>H</mi><mo>×</mo><mi>R</mi></mrow><annotation encoding="application/x-tex"> 2 \times H \times R </annotation></semantics></math>, where <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>R</mi></mrow><annotation encoding="application/x-tex"> R </annotation></semantics></math> is the initial reward rate per slot. After 32 halvings, the minted amount will be very close to this value.
To mint the total supply <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi></mrow><annotation encoding="application/x-tex"> T </annotation></semantics></math> in about a week:
<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><semantics><mrow><mi>T</mi><mo>≈</mo><mn>2</mn><mo>×</mo><mi>H</mi><mo>×</mo><mi>R</mi></mrow><annotation encoding="application/x-tex">T \approx 2 \times H \times R</annotation></semantics></math>
<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><semantics><mrow><mi>R</mi><mo>=</mo><mfrac><mi>T</mi><mrow><mn>2</mn><mo>×</mo><mi>H</mi></mrow></mfrac></mrow><annotation encoding="application/x-tex">R = \frac{T}{2 \times H}</annotation></semantics></math>

For example, if your total supply <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi><mo>=</mo><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> T = 1,000,000 </annotation></semantics></math> tokens:
<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><semantics><mrow><mi>R</mi><mo>=</mo><mfrac><mrow><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><mrow><mn>2</mn><mo>×</mo><mn>47</mn><mo separator="true">,</mo><mn>250</mn></mrow></mfrac><mo>=</mo><mfrac><mrow><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><mrow><mn>94</mn><mo separator="true">,</mo><mn>500</mn></mrow></mfrac><mo>≈</mo><mn>10.58</mn><mtext> tokens per slot</mtext></mrow><annotation encoding="application/x-tex">R = \frac{1,000,000}{2 \times 47,250} = \frac{1,000,000}{94,500} \approx 10.58 \text{ tokens per slot}</annotation></semantics></math>

If the game requires <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>R</mi></mrow><annotation encoding="application/x-tex"> R </annotation></semantics></math> to be an integer, round it (e.g., to 11 tokens per slot) and adjust <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi></mrow><annotation encoding="application/x-tex"> T </annotation></semantics></math> or <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>H</mi></mrow><annotation encoding="application/x-tex"> H </annotation></semantics></math> slightly if needed. Alternatively, if your token supports decimals (e.g., 9 decimal places like many SPL tokens), set <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>R</mi><mo>=</mo><mn>10.58</mn></mrow><annotation encoding="application/x-tex"> R = 10.58 </annotation></semantics></math> or scale it appropriately (e.g., <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>R</mi><mo>=</mo><mn>10</mn><mo separator="true">,</mo><mn>580</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> R = 10,580,000,000 </annotation></semantics></math> units for a token with 9 decimals).


Set a Dust Threshold to Stop Minting

To ensure minting stops when the remaining supply is negligible, configure a dust threshold. For example:

Set <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mtext>dust_threshold_divisor</mtext><mo>=</mo><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> \text{dust\_threshold\_divisor} = 1,000,000 </annotation></semantics></math>.
Minting will stop when the remaining supply falls below <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mfrac><mi>T</mi><mrow><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow></mfrac></mrow><annotation encoding="application/x-tex"> \frac{T}{1,000,000} </annotation></semantics></math> (e.g., 1 token if <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi><mo>=</mo><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> T = 1,000,000 </annotation></semantics></math>).


This ensures the total supply is effectively minted without leaving tiny fractions unclaimed.


Implement the Parameters in the Game

In your game’s initialization code (e.g., an initialize_program function), configure:

halving_interval: 47,250 slots.
initial_reward_rate: <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>R</mi></mrow><annotation encoding="application/x-tex"> R </annotation></semantics></math> (e.g., 11 tokens per slot, or a scaled value if using decimals).
dust_threshold_divisor: 1,000,000 (or adjust based on your preference).


Ensure the game tracks the total minted tokens and applies the halving logic every 47,250 slots, reducing the reward rate by half up to 32 times.




Example Configuration
If your total supply is <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi><mo>=</mo><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> T = 1,000,000 </annotation></semantics></math> tokens:

Slots in a week: 1,512,000 slots.
Halving interval (<math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>H</mi></mrow><annotation encoding="application/x-tex"> H </annotation></semantics></math>): 47,250 slots.
Initial reward rate (<math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>R</mi></mrow><annotation encoding="application/x-tex"> R </annotation></semantics></math>): <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mfrac><mrow><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><mrow><mn>2</mn><mo>×</mo><mn>47</mn><mo separator="true">,</mo><mn>250</mn></mrow></mfrac><mo>≈</mo><mn>10.58</mn></mrow><annotation encoding="application/x-tex"> \frac{1,000,000}{2 \times 47,250} \approx 10.58 </annotation></semantics></math> tokens per slot (round to 11 if integers are required, making <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi><mo>≈</mo><mn>1</mn><mo separator="true">,</mo><mn>039</mn><mo separator="true">,</mo><mn>500</mn></mrow><annotation encoding="application/x-tex"> T \approx 1,039,500 </annotation></semantics></math>).
Dust threshold: Stop minting when remaining supply < 1 token (if <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mtext>dust_threshold_divisor</mtext><mo>=</mo><mn>1</mn><mo separator="true">,</mo><mn>000</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> \text{dust\_threshold\_divisor} = 1,000,000 </annotation></semantics></math>).

After 32 halvings (which takes <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mn>32</mn><mo>×</mo><mn>47</mn><mo separator="true">,</mo><mn>250</mn><mo>=</mo><mn>1</mn><mo separator="true">,</mo><mn>512</mn><mo separator="true">,</mo><mn>000</mn></mrow><annotation encoding="application/x-tex"> 32 \times 47,250 = 1,512,000 </annotation></semantics></math> slots, or one week), the reward rate drops to <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mfrac><mi>R</mi><msup><mn>2</mn><mn>32</mn></msup></mfrac></mrow><annotation encoding="application/x-tex"> \frac{R}{2^{32}} </annotation></semantics></math> (e.g., <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mfrac><mn>10.58</mn><mrow><mn>4</mn><mo separator="true">,</mo><mn>294</mn><mo separator="true">,</mo><mn>967</mn><mo separator="true">,</mo><mn>296</mn></mrow></mfrac><mo>≈</mo><mn>0</mn></mrow><annotation encoding="application/x-tex"> \frac{10.58}{4,294,967,296} \approx 0 </annotation></semantics></math>), and nearly all of <math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow><mi>T</mi></mrow><annotation encoding="application/x-tex"> T </annotation></semantics></math> will have been minted.




 ts-node cli/program.ts initialize-program -k ~/.config/solana/aipool_test.json -m EGtEL3wUcAAiZ9oZrHGKbcC71XgRcZ2QGw8MLWurysBL -f 8kvqgxQG77pv6RvEou8f2kHSWi3rtx8F7MksXUqNLGmn -h 47250 -t 21000000000000 -i 11 -c 100