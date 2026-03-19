const answers = document.querySelectorAll(".answer");

answers.forEach(answer => {
  answer.addEventListener("click", () => {
    // remove active from all
    answers.forEach(a => a.classList.remove("active"));
    
    // add active to clicked one
    answer.classList.add("active");
  });
});