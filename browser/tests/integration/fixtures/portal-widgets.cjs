/** Shared established portal widgets exercise canonical and diagnostic reader compatibility. */
module.exports = `<!doctype html><title>Portal widgets</title>
<div class="datepicker datepicker-dropdown">
  <div class="datepicker-days">
    <table class="table-condensed">
      <thead><tr><th>Su</th><th>Mo</th></tr></thead>
      <tbody><tr><td class="old day">22</td><td class="day">23</td></tr></tbody>
    </table>
  </div>
</div>
<ul class="results">
  <li><button id="pickPayer0"><b>PAYER</b> - CA</button></li>
</ul>
<table><tr class="result-row">
  <td>Jordan Example Provider NPI 0000000000 In Network: Y</td>
  <td><button id="selectProvider0" name="selectProvider0"></button></td>
</tr></table>
<button id="npiSearch"><i class="fa fa-search"></i></button>
<button id="spriteButton"><svg><use href="#icon-trash"></use></svg></button>
<div class="rating">
  <input type="radio" name="stars" id="s1" style="display:none"><label for="s1">★</label>
  <input type="radio" name="stars" id="s2" style="display:none" checked><label for="s2">★</label>
</div>
<div class="suggestions" style="width:840px">
  <div class="row" style="cursor:pointer;padding:8px"><div>Tartine Bakery</div><div>600 Guerrero St</div></div>
  <div class="row" style="cursor:pointer;padding:8px"><div>Tartine Manufactory</div><div>595 Alabama St</div></div>
</div>
<div class="panel" style="cursor:pointer;width:840px;height:400px"><p>A whole panel is not one control</p></div>`;
